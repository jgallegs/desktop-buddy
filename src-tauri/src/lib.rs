// Joaquincillo — punto de entrada de la app Tauri.
//
// Responsabilidades de este módulo:
//   - Crear la ventana flotante y anclarla abajo a la izquierda.
//   - Hacer la zona transparente "click-through" salvo donde está el sprite.
//   - Montar el icono de bandeja (tray) con su menú.
//   - Lanzar los monitores en segundo plano (actividad de teclado y calendario).
//
// Los monitores emiten eventos hacia el frontend mediante `app.emit(...)`.
// El frontend (states.js) escucha esos eventos y cambia la animación / frase.

mod activity;
mod calendar;
mod jira;
mod secrets;
mod settings;
mod updater;

use serde::{Deserialize, Serialize};
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent,
};

use std::sync::atomic::{AtomicBool, Ordering};

// Margen (en px) respecto a las esquinas de la pantalla.
const MARGEN_X: i32 = 24;
// 0 = pegado al fondo, para que el sprite quede por encima de la barra de tareas.
const MARGEN_Y: i32 = 0;

// Anchos de la ventana: normal y mientras duerme (la cama es ancha).
const ANCHO_NORMAL: u32 = 280;
const ANCHO_DORMIR: u32 = 380;
const ALTO_VENTANA: u32 = 440;

// ¿Está durmiendo? Lo usa el hilo de hover para ampliar la zona clicable.
static DORMIDO: AtomicBool = AtomicBool::new(false);

/// Ancla la ventana abajo a la izquierda, con los pies del sprite justo sobre el
/// borde SUPERIOR de la barra de tareas (la barra es "su suelo"). Para eso usa el
/// área de trabajo de Windows (pantalla menos la barra). Así no se solapa con la
/// barra y no hay pelea de z-order (que causaba el parpadeo).
fn anclar_abajo_izquierda(win: &tauri::WebviewWindow) {
    let Ok(size) = win.outer_size() else { return };

    #[cfg(windows)]
    {
        use windows::Win32::Foundation::RECT;
        use windows::Win32::UI::WindowsAndMessaging::{
            SystemParametersInfoW, SPI_GETWORKAREA, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
        };
        let mut wa = RECT::default();
        let ok = unsafe {
            SystemParametersInfoW(
                SPI_GETWORKAREA,
                0,
                Some(&mut wa as *mut RECT as *mut core::ffi::c_void),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
            )
        }
        .is_ok();
        if ok && wa.bottom > wa.top {
            let x = wa.left + MARGEN_X;
            let y = wa.bottom - size.height as i32; // pies sobre el borde de la barra
            let _ = win.set_position(tauri::PhysicalPosition::new(x, y.max(0)));
            return;
        }
    }

    // Fallback (no-Windows o si falla): usar el alto del monitor completo.
    if let Ok(Some(monitor)) = win.current_monitor() {
        let scr = monitor.size();
        let x = MARGEN_X;
        let y = (scr.height as i32) - (size.height as i32) - MARGEN_Y;
        let _ = win.set_position(tauri::PhysicalPosition::new(x, y.max(0)));
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // FIX del parpadeo/desaparición: Chromium (WebView2) tiene la función
    // "CalculateNativeWinOcclusion" que en ventanas transparentes y always-on-top
    // cree por error que están tapadas y DEJA DE PINTARLAS. La desactivamos para que
    // Joaquincillo no desaparezca. Debe hacerse ANTES de crear el WebView.
    std::env::set_var(
        "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS",
        "--disable-features=CalculateNativeWinOcclusion",
    );

    tauri::Builder::default()
        .setup(|app| {
            let win = app
                .get_webview_window("mascota")
                .expect("falta la ventana 'mascota'");

            // Posición inicial: abajo a la izquierda.
            anclar_abajo_izquierda(&win);

            // Cargar ajustes (config.json en la carpeta de datos de la app).
            let config_dir = app.path().app_config_dir().unwrap_or_else(|_| ".".into());
            let mut cfg = settings::cargar(&config_dir);
            // Mover tokens en texto plano al llavero cifrado de Windows (y borrarlos del archivo).
            settings::migrar_secretos(&config_dir, &mut cfg);

            // --- Icono de bandeja con menú ---
            let ajustes = MenuItem::with_id(app, "ajustes", "Ajustes…", true, None::<&str>)?;
            let actualizar = MenuItem::with_id(app, "actualizar", "Buscar actualizaciones", true, None::<&str>)?;
            let pausar = MenuItem::with_id(app, "pausar", "Pausar / reanudar", true, None::<&str>)?;
            let esconder = MenuItem::with_id(app, "esconder", "Mostrar / esconder", true, None::<&str>)?;
            let salir = MenuItem::with_id(app, "salir", "Salir", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&esconder, &ajustes, &actualizar, &pausar, &salir])?;

            let cfg_tray = cfg.clone(); // para el botón "Buscar actualizaciones"
            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .tooltip("Joaquincillo")
                .on_menu_event(move |app, event| match event.id.as_ref() {
                    "salir" => app.exit(0),
                    "esconder" => {
                        if let Some(w) = app.get_webview_window("mascota") {
                            let visible = w.is_visible().unwrap_or(true);
                            let _ = if visible { w.hide() } else { w.show() };
                        }
                    }
                    "pausar" => {
                        // Avisa al frontend para que pause/reanude las animaciones.
                        let _ = app.emit("mascota://toggle-pausa", ());
                    }
                    "actualizar" => {
                        // Comprobación bajo demanda: si hay update, descarga y reinicia.
                        updater::comprobar_ahora(app.clone(), cfg_tray.clone());
                    }
                    "ajustes" => abrir_ajustes(app),
                    _ => {}
                })
                .build(app)?;

            // --- Monitores en segundo plano ---
            // Detección de actividad de teclado -> evento "mascota://on-fire".
            activity::iniciar_monitor(app.handle().clone(), cfg.on_fire_apm);

            // Sondeo del calendario (Microsoft Graph) -> evento "mascota://reunion".
            calendar::iniciar_monitor(app.handle().clone(), cfg.clone());

            // Sondeo de Jira Cloud -> evento "mascota://jira".
            jira::iniciar_monitor(app.handle().clone(), cfg.clone());

            // Auto-actualización con Velopack (si hay github_repo configurado).
            updater::iniciar_monitor(app.handle().clone(), cfg.clone());

            // Click-through inteligente: la ventana deja pasar los clicks a lo que hay
            // debajo, SALVO cuando el cursor está sobre el sprite (para poder pulsarlo).
            iniciar_hover_clickthrough(win.clone());

            Ok(())
        })
        // Comandos invocables desde el frontend (window.__TAURI__.core.invoke).
        .invoke_handler(tauri::generate_handler![
            comando_esconder,
            comando_set_clickthrough,
            comando_set_dormido,
            cargar_ajustes,
            guardar_ajustes,
        ])
        .on_window_event(|window, event| {
            // La mascota no se cierra: se esconde (la app sigue viva en la bandeja).
            // Las demás ventanas (ajustes) se cierran con normalidad.
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "mascota" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error al arrancar Joaquincillo");
}

use tauri::Emitter;

// Caja (en px, relativa a la esquina superior izquierda de la ventana de 280x440)
// que ocupa el sprite dibujado. Solo dentro de esta caja la ventana captura el ratón.
const SPRITE_X0: i32 = 40;
const SPRITE_X1: i32 = 240;
const SPRITE_Y0: i32 = 60;
const SPRITE_Y1: i32 = 440;

/// Sondea (cada 120 ms) la posición global del cursor y activa/desactiva el
/// "click-through" según esté o no sobre el sprite. Es nativo y muy barato, y a
/// diferencia de los eventos del WebView funciona también cuando la ventana ya es
/// transparente a los clicks.
fn iniciar_hover_clickthrough(win: tauri::WebviewWindow) {
    #[cfg(windows)]
    std::thread::spawn(move || {
        use windows::Win32::Foundation::POINT;
        use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

        let mut ignorando = false;
        // Empezamos dejando pasar los clicks.
        let _ = win.set_ignore_cursor_events(true);

        loop {
            std::thread::sleep(std::time::Duration::from_millis(120));
            if !win.is_visible().unwrap_or(false) {
                continue;
            }
            let mut p = POINT::default();
            if unsafe { GetCursorPos(&mut p) }.is_err() {
                continue;
            }
            let (Ok(pos), Ok(size)) = (win.outer_position(), win.outer_size()) else {
                continue;
            };
            // Coordenadas del cursor relativas a la ventana.
            let rx = p.x - pos.x;
            let ry = p.y - pos.y;
            let dentro_ventana = rx >= 0 && ry >= 0 && rx <= size.width as i32 && ry <= size.height as i32;
            // Durmiendo: toda la ventana es clicable (para tocar la cama y despertarlo).
            let sobre_sprite = if DORMIDO.load(Ordering::Relaxed) {
                dentro_ventana
            } else {
                dentro_ventana
                    && rx >= SPRITE_X0 && rx <= SPRITE_X1
                    && ry >= SPRITE_Y0 && ry <= SPRITE_Y1
            };

            // Si está sobre el sprite -> captura clicks (ignore = false).
            let nuevo_ignorar = !sobre_sprite;
            if nuevo_ignorar != ignorando {
                ignorando = nuevo_ignorar;
                let _ = win.set_ignore_cursor_events(ignorando);
            }
        }
    });
}

/// Esconde la ventana de la mascota.
#[tauri::command]
fn comando_esconder(app: tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("mascota") {
        let _ = w.hide();
    }
}

/// Abre (o trae al frente) la ventana de ajustes.
fn abrir_ajustes(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("ajustes") {
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    let _ = WebviewWindowBuilder::new(app, "ajustes", WebviewUrl::App("ajustes.html".into()))
        .title("Ajustes de Joaquincillo")
        .inner_size(470.0, 600.0)
        .resizable(true)
        .build();
}

// --- Panel de ajustes (ventana "ajustes") -------------------------------------

/// Lo que ve el formulario. El token NO se devuelve; solo si existe o no.
#[derive(Serialize)]
struct AjustesVista {
    jira_site: String,
    jira_email: String,
    jira_token_set: bool,
    jira_proyecto: String,
    jira_intervalo_s: u64,
    azure_client_id: String,
    azure_tenant: String,
    on_fire_apm: u64,
    aviso_reunion_min: i64,
    github_repo: String,
}

/// Lo que envía el formulario al guardar.
#[derive(Deserialize)]
struct AjustesEntrada {
    jira_site: String,
    jira_email: String,
    jira_token: String, // vacío = no cambiar
    jira_proyecto: String,
    jira_intervalo_s: u64,
    azure_client_id: String,
    azure_tenant: String,
    on_fire_apm: u64,
    aviso_reunion_min: i64,
    github_repo: String,
}

#[tauri::command]
fn cargar_ajustes(app: tauri::AppHandle) -> AjustesVista {
    let dir = app.path().app_config_dir().unwrap_or_else(|_| ".".into());
    let s = settings::cargar(&dir);
    AjustesVista {
        jira_site: s.jira_site,
        jira_email: s.jira_email,
        jira_token_set: secrets::obtener("jira_token").map(|t| !t.is_empty()).unwrap_or(false),
        jira_proyecto: s.jira_proyecto,
        jira_intervalo_s: s.jira_intervalo_s,
        azure_client_id: s.azure_client_id,
        azure_tenant: s.azure_tenant,
        on_fire_apm: s.on_fire_apm,
        aviso_reunion_min: s.aviso_reunion_min,
        github_repo: s.github_repo,
    }
}

#[tauri::command]
fn guardar_ajustes(app: tauri::AppHandle, datos: AjustesEntrada) -> Result<(), String> {
    let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    let mut s = settings::cargar(&dir);
    s.jira_site = datos.jira_site;
    s.jira_email = datos.jira_email;
    s.jira_proyecto = datos.jira_proyecto;
    s.jira_intervalo_s = datos.jira_intervalo_s;
    s.azure_client_id = datos.azure_client_id;
    s.azure_tenant = datos.azure_tenant;
    s.on_fire_apm = datos.on_fire_apm;
    s.aviso_reunion_min = datos.aviso_reunion_min;
    s.github_repo = datos.github_repo;

    // Token: solo si el usuario escribió uno nuevo. Va al llavero cifrado.
    let token_nuevo = datos.jira_token.trim();
    if !token_nuevo.is_empty() {
        secrets::guardar("jira_token", token_nuevo).map_err(|e| e.to_string())?;
    }

    settings::escribir(&dir, &s).map_err(|e| e.to_string())?;

    if let Some(w) = app.get_webview_window("ajustes") {
        let _ = w.close();
    }
    Ok(())
}

/// Activa/desactiva el "click-through": cuando está activo, los clicks atraviesan
/// la ventana y van a la app que hay debajo. El frontend lo activa cuando el ratón
/// NO está sobre el sprite, y lo desactiva cuando sí lo está, para poder interactuar.
#[tauri::command]
fn comando_set_clickthrough(app: tauri::AppHandle, ignorar: bool) {
    if let Some(w) = app.get_webview_window("mascota") {
        let _ = w.set_ignore_cursor_events(ignorar);
    }
}

/// Ensancha/estrecha la ventana al dormir/despertar (la escena de la cama es ancha).
/// La ventana está anclada abajo a la izquierda, así que al crecer se extiende hacia
/// la derecha sin moverse.
#[tauri::command]
fn comando_set_dormido(app: tauri::AppHandle, dormido: bool) {
    DORMIDO.store(dormido, Ordering::Relaxed);
    if let Some(w) = app.get_webview_window("mascota") {
        let ancho = if dormido { ANCHO_DORMIR } else { ANCHO_NORMAL };
        let _ = w.set_size(tauri::PhysicalSize::new(ancho, ALTO_VENTANA));
    }
}
