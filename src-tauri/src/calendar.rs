// Calendario de Outlook/Teams vía Microsoft Graph — estado "meeting_soon".
//
// Autenticación: flujo "device code" (sin secreto de cliente, ideal para escritorio).
// Al arrancar, si no hay sesión guardada, Joaquincillo muestra en su bocadillo un
// código y una URL: el usuario entra en la URL, mete el código y autoriza. A partir
// de ahí se guarda el token (con su refresh_token) y se renueva solo.
//
// Si config.json no tiene azure_client_id, se ejecuta en modo SIMULACIÓN (una reunión
// de prueba a los 20 s) para poder ver el comportamiento sin montar nada de Azure.

use crate::settings::{self, Settings};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

const INTERVALO_SONDEO_S: u64 = 60;
const GRAPH_SCOPE: &str = "offline_access Calendars.Read User.Read";

/// Reunión que se envía al frontend.
#[derive(Clone, Serialize)]
pub struct Reunion {
    pub titulo: String,
    pub minutos: i64,
}

/// Instrucción de login que se envía al frontend (para mostrarla en el bocadillo).
#[derive(Clone, Serialize)]
pub struct Login {
    pub codigo: String,
    pub url: String,
}

/// Token guardado en disco (ms_token.json).
#[derive(Clone, Serialize, Deserialize)]
struct TokenGuardado {
    access_token: String,
    refresh_token: String,
    /// Epoch (segundos) en el que caduca el access_token.
    expira_en: i64,
}

pub fn iniciar_monitor(app: AppHandle, cfg: Settings, config_dir: PathBuf) {
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("no se pudo crear el runtime de tokio");

        rt.block_on(async move {
            // --- Modo simulación: sin client_id no hay Graph ---
            if !cfg.calendario_configurado() {
                tokio::time::sleep(Duration::from_secs(20)).await;
                let _ = app.emit(
                    "mascota://reunion",
                    Reunion { titulo: "Daily del equipo".into(), minutos: cfg.aviso_reunion_min },
                );
                return;
            }

            let cliente = reqwest::Client::new();
            let token_path = settings::ruta_token(&config_dir);

            loop {
                // 1) Asegurar que tenemos un token válido (login o refresh).
                let token = match asegurar_token(&app, &cliente, &cfg, &token_path).await {
                    Ok(t) => t,
                    Err(e) => {
                        eprintln!("[calendar] no se pudo autenticar: {e}");
                        tokio::time::sleep(Duration::from_secs(30)).await;
                        continue;
                    }
                };

                // 2) Consultar la próxima reunión.
                match proxima_reunion(&cliente, &token).await {
                    Ok(Some(r)) if r.minutos <= cfg.aviso_reunion_min && r.minutos >= 0 => {
                        let _ = app.emit("mascota://reunion", r);
                    }
                    Ok(_) => {}
                    Err(e) => eprintln!("[calendar] error consultando Graph: {e}"),
                }

                tokio::time::sleep(Duration::from_secs(INTERVALO_SONDEO_S)).await;
            }
        });
    });
}

/// Devuelve un access_token válido: lo lee de disco, lo refresca si caduca pronto,
/// o lanza el flujo device-code si no hay sesión.
async fn asegurar_token(
    app: &AppHandle,
    cliente: &reqwest::Client,
    cfg: &Settings,
    token_path: &PathBuf,
) -> Result<String, Box<dyn std::error::Error>> {
    let ahora = chrono::Utc::now().timestamp();

    // ¿Hay token guardado?
    if let Ok(txt) = std::fs::read_to_string(token_path) {
        if let Ok(tk) = serde_json::from_str::<TokenGuardado>(&txt) {
            if tk.expira_en - ahora > 120 {
                return Ok(tk.access_token); // todavía válido
            }
            // Caduca pronto: intentar refrescar.
            if let Ok(nuevo) = refrescar(cliente, cfg, &tk.refresh_token).await {
                guardar(token_path, &nuevo);
                return Ok(nuevo.access_token);
            }
        }
    }

    // No hay token (o el refresh falló): flujo device-code.
    let nuevo = device_code(app, cliente, cfg).await?;
    guardar(token_path, &nuevo);
    Ok(nuevo.access_token)
}

fn guardar(token_path: &PathBuf, tk: &TokenGuardado) {
    if let Ok(txt) = serde_json::to_string(tk) {
        let _ = std::fs::write(token_path, txt);
    }
}

#[derive(Deserialize)]
struct DeviceCodeResp {
    device_code: String,
    user_code: String,
    verification_uri: String,
    interval: Option<u64>,
}

#[derive(Deserialize)]
struct TokenResp {
    access_token: Option<String>,
    refresh_token: Option<String>,
    expires_in: Option<i64>,
    error: Option<String>,
}

/// Flujo device-code completo: pide código, lo muestra en el bocadillo y hace polling
/// hasta que el usuario autoriza.
async fn device_code(
    app: &AppHandle,
    cliente: &reqwest::Client,
    cfg: &Settings,
) -> Result<TokenGuardado, Box<dyn std::error::Error>> {
    let base = format!("https://login.microsoftonline.com/{}/oauth2/v2.0", cfg.azure_tenant);

    let dc: DeviceCodeResp = cliente
        .post(format!("{base}/devicecode"))
        .form(&[("client_id", cfg.azure_client_id.as_str()), ("scope", GRAPH_SCOPE)])
        .send().await?
        .json().await?;

    // Mostrar al usuario el código y la URL en el bocadillo de la mascota.
    let _ = app.emit(
        "mascota://login",
        Login { codigo: dc.user_code.clone(), url: dc.verification_uri.clone() },
    );

    let intervalo = dc.interval.unwrap_or(5).max(1);
    loop {
        tokio::time::sleep(Duration::from_secs(intervalo)).await;

        let tr: TokenResp = cliente
            .post(format!("{base}/token"))
            .form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("client_id", cfg.azure_client_id.as_str()),
                ("device_code", dc.device_code.as_str()),
            ])
            .send().await?
            .json().await?;

        match tr.error.as_deref() {
            Some("authorization_pending") => continue, // el usuario aún no ha autorizado
            Some("slow_down") => { tokio::time::sleep(Duration::from_secs(intervalo)).await; }
            Some(e) => return Err(format!("device-code: {e}").into()),
            None => {
                if let (Some(at), Some(rt), Some(exp)) = (tr.access_token, tr.refresh_token, tr.expires_in) {
                    let _ = app.emit("mascota://login-ok", ());
                    return Ok(TokenGuardado {
                        access_token: at,
                        refresh_token: rt,
                        expira_en: chrono::Utc::now().timestamp() + exp,
                    });
                }
            }
        }
    }
}

/// Renueva el access_token con el refresh_token.
async fn refrescar(
    cliente: &reqwest::Client,
    cfg: &Settings,
    refresh_token: &str,
) -> Result<TokenGuardado, Box<dyn std::error::Error>> {
    let base = format!("https://login.microsoftonline.com/{}/oauth2/v2.0", cfg.azure_tenant);
    let tr: TokenResp = cliente
        .post(format!("{base}/token"))
        .form(&[
            ("grant_type", "refresh_token"),
            ("client_id", cfg.azure_client_id.as_str()),
            ("refresh_token", refresh_token),
            ("scope", GRAPH_SCOPE),
        ])
        .send().await?
        .json().await?;

    match (tr.access_token, tr.expires_in) {
        (Some(at), Some(exp)) => Ok(TokenGuardado {
            access_token: at,
            // Si Microsoft no devuelve un refresh nuevo, reusar el anterior.
            refresh_token: tr.refresh_token.unwrap_or_else(|| refresh_token.to_string()),
            expira_en: chrono::Utc::now().timestamp() + exp,
        }),
        _ => Err("refresh sin access_token".into()),
    }
}

/// Consulta Graph y devuelve la próxima reunión (la más cercana en la próxima hora).
async fn proxima_reunion(
    cliente: &reqwest::Client,
    token: &str,
) -> Result<Option<Reunion>, Box<dyn std::error::Error>> {
    let ahora = chrono::Utc::now();
    let fin = ahora + chrono::Duration::hours(1);
    let url = format!(
        "https://graph.microsoft.com/v1.0/me/calendarView?startDateTime={}&endDateTime={}&$orderby=start/dateTime&$top=3&$select=subject,start,isCancelled",
        ahora.to_rfc3339(),
        fin.to_rfc3339()
    );

    let resp: serde_json::Value = cliente
        .get(url)
        .bearer_auth(token)
        .header("Prefer", "outlook.timezone=\"UTC\"")
        .send().await?
        .json().await?;

    let eventos = match resp.get("value").and_then(|v| v.as_array()) {
        Some(v) => v,
        None => return Ok(None),
    };

    for ev in eventos {
        if ev.get("isCancelled").and_then(|c| c.as_bool()).unwrap_or(false) {
            continue;
        }
        let titulo = ev.get("subject").and_then(|s| s.as_str()).unwrap_or("Reunión").to_string();
        let inicio_str = ev
            .get("start")
            .and_then(|s| s.get("dateTime"))
            .and_then(|d| d.as_str());
        if let Some(s) = inicio_str {
            // Graph devuelve sin zona si pedimos UTC; lo tratamos como UTC.
            if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(&s[..19.min(s.len())], "%Y-%m-%dT%H:%M:%S") {
                let inicio = naive.and_utc();
                let minutos = (inicio - ahora).num_minutes();
                if minutos >= 0 {
                    return Ok(Some(Reunion { titulo, minutos }));
                }
            }
        }
    }
    Ok(None)
}
