// Novedades de Jira Cloud — Joaquincillo te las cuenta en su bocadillo.
//
// Auth: token de API personal (Basic auth = email:token en base64, lo hace reqwest).
// Sondeo: cada `jira_intervalo_s` segundos busca incidencias actualizadas desde el
// último sondeo (las tuyas y, opcionalmente, las de un proyecto). Por cada novedad
// decide el tipo (cambio de estado / comentario / mención) y emite `mascota://jira`.
//
// "Tiempo real": una app de escritorio no puede recibir push de Jira sin un servidor
// con URL pública (webhooks). Esto es sondeo frecuente, que se siente casi en directo.
//
// Si config.json no tiene credenciales de Jira, este monitor no hace nada.

use crate::settings::Settings;
use serde::Serialize;
use std::collections::HashSet;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

/// Aviso de Jira que se envía al frontend.
#[derive(Clone, Serialize)]
pub struct AvisoJira {
    pub texto: String,
}

pub fn iniciar_monitor(app: AppHandle, cfg: Settings) {
    if !cfg.jira_configurado() {
        return; // sin credenciales, nada que hacer
    }

    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("no se pudo crear el runtime de tokio (jira)");

        rt.block_on(async move {
            let cliente = reqwest::Client::new();
            let base = format!("https://{}", cfg.jira_site.trim_end_matches('/'));
            let intervalo = cfg.jira_intervalo_s.max(20);
            // Ventana de búsqueda (minutos): el intervalo + 1 de margen.
            let mins = (intervalo / 60).max(1) + 1;

            // Mi accountId (para detectar menciones y excluir mis propios comentarios).
            let mi_id = match mi_account_id(&cliente, &cfg, &base).await {
                Ok(id) => id,
                Err(e) => { eprintln!("[jira] no se pudo identificar al usuario: {e}"); String::new() }
            };

            // Para no repetir el mismo aviso (clave: ISSUE@updated).
            let mut vistos: HashSet<String> = HashSet::new();
            let mut primera_vuelta = true;

            loop {
                match sondear(&cliente, &cfg, &base, mins).await {
                    Ok(issues) => {
                        for ev in &issues {
                            let clave = format!("{}@{}", ev.key, ev.updated);
                            if !vistos.insert(clave) {
                                continue; // ya avisado
                            }
                            // En la primera vuelta no avisamos del historial existente,
                            // solo "sembramos" lo ya visto para avisar de aquí en adelante.
                            if primera_vuelta {
                                continue;
                            }
                            if let Some(texto) = mensaje(ev, &mi_id, mins) {
                                let _ = app.emit("mascota://jira", AvisoJira { texto });
                            }
                        }
                        primera_vuelta = false;
                        // Evitar que `vistos` crezca sin límite.
                        if vistos.len() > 500 { vistos.clear(); primera_vuelta = true; }
                    }
                    Err(e) => eprintln!("[jira] error sondeando: {e}"),
                }
                tokio::time::sleep(Duration::from_secs(intervalo)).await;
            }
        });
    });
}

/// Datos de una incidencia actualizada.
struct IssueEv {
    key: String,
    summary: String,
    status: String,
    updated: String,
    /// Último comentario: (accountId autor, json del cuerpo, created).
    ultimo_comentario: Option<(String, String, String)>,
}

async fn mi_account_id(
    cliente: &reqwest::Client,
    cfg: &Settings,
    base: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let v: serde_json::Value = cliente
        .get(format!("{base}/rest/api/3/myself"))
        .basic_auth(&cfg.jira_email, Some(&cfg.jira_token))
        .header("Accept", "application/json")
        .send().await?
        .json().await?;
    Ok(v.get("accountId").and_then(|a| a.as_str()).unwrap_or("").to_string())
}

async fn sondear(
    cliente: &reqwest::Client,
    cfg: &Settings,
    base: &str,
    mins: u64,
) -> Result<Vec<IssueEv>, Box<dyn std::error::Error>> {
    let proyecto = cfg.jira_proyecto.trim();
    let jql = if proyecto.is_empty() {
        format!("assignee = currentUser() AND updated >= \"-{mins}m\" ORDER BY updated DESC")
    } else {
        format!("(assignee = currentUser() OR project = {proyecto}) AND updated >= \"-{mins}m\" ORDER BY updated DESC")
    };

    let body = serde_json::json!({
        "jql": jql,
        "fields": ["summary", "status", "updated", "comment"],
        "maxResults": 20
    });

    let resp: serde_json::Value = cliente
        .post(format!("{base}/rest/api/3/search/jql"))
        .basic_auth(&cfg.jira_email, Some(&cfg.jira_token))
        .header("Accept", "application/json")
        .json(&body)
        .send().await?
        .json().await?;

    let mut out = Vec::new();
    if let Some(arr) = resp.get("issues").and_then(|v| v.as_array()) {
        for issue in arr {
            let key = issue.get("key").and_then(|k| k.as_str()).unwrap_or("").to_string();
            let f = issue.get("fields").cloned().unwrap_or_default();
            let summary = f.get("summary").and_then(|s| s.as_str()).unwrap_or("").to_string();
            let status = f.get("status").and_then(|s| s.get("name")).and_then(|n| n.as_str()).unwrap_or("").to_string();
            let updated = f.get("updated").and_then(|u| u.as_str()).unwrap_or("").to_string();

            // Último comentario (si lo hay).
            let ultimo_comentario = f
                .get("comment")
                .and_then(|c| c.get("comments"))
                .and_then(|c| c.as_array())
                .and_then(|arr| arr.last())
                .map(|c| {
                    let autor = c.get("author").and_then(|a| a.get("accountId")).and_then(|a| a.as_str()).unwrap_or("").to_string();
                    let cuerpo = c.get("body").map(|b| b.to_string()).unwrap_or_default();
                    let created = c.get("created").and_then(|x| x.as_str()).unwrap_or("").to_string();
                    (autor, cuerpo, created)
                });

            if !key.is_empty() {
                out.push(IssueEv { key, summary, status, updated, ultimo_comentario });
            }
        }
    }
    Ok(out)
}

/// Construye el texto del aviso según el tipo de novedad.
fn mensaje(ev: &IssueEv, mi_id: &str, mins: u64) -> Option<String> {
    let resumen = recortar(&ev.summary, 40);

    // ¿El último comentario es reciente (dentro de la ventana) y no es mío?
    if let Some((autor, cuerpo, created)) = &ev.ultimo_comentario {
        if comentario_reciente(created, mins) && autor != mi_id {
            if !mi_id.is_empty() && cuerpo.contains(mi_id) {
                return Some(format!("💬 Te han mencionado en {} — {}", ev.key, resumen));
            }
            return Some(format!("💬 Nuevo comentario en {} — {}", ev.key, resumen));
        }
    }

    // Si no hay comentario nuevo, lo tratamos como actualización (estado/campos).
    if ev.status.is_empty() {
        Some(format!("✏️ {} actualizada — {}", ev.key, resumen))
    } else {
        Some(format!("🔄 {} → {} — {}", ev.key, ev.status, resumen))
    }
}

/// ¿El comentario se creó dentro de los últimos `mins` minutos?
fn comentario_reciente(created: &str, mins: u64) -> bool {
    // created viene como "2026-06-09T10:30:00.000+0200".
    if let Ok(t) = chrono::DateTime::parse_from_str(created, "%Y-%m-%dT%H:%M:%S%.3f%z") {
        let diff = chrono::Utc::now().signed_duration_since(t.with_timezone(&chrono::Utc));
        return diff.num_minutes() <= mins as i64 && diff.num_minutes() >= -1;
    }
    false
}

fn recortar(s: &str, max: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let corto: String = s.chars().take(max - 1).collect();
        format!("{corto}…")
    }
}
