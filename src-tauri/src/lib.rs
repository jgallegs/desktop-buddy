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
mod settings;
mod updater;

use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager, WindowEvent,
};

use std::sync::atomic::{AtomicBool, Ordering};

// Margen (en px) respecto a las esquinas de la pantalla.
const MARGEN_X: i32 = 24;
const MARGEN_Y: i32 = 24;

// Anchos de la ventana: normal y mientras duerme (la cama es ancha).
const ANCHO_NORMAL: u32 = 280;
const ANCHO_DORMIR: u32 = 380;
const ALTO_VENTANA: u32 = 440;

// ¿Está durmiendo? Lo usa el hilo de hover para ampliar la zona clicable.
static DORMIDO: AtomicBool = AtomicBool::new(false);

/// Coloca la ventana de la mascota anclada en la esquina inferior izquierda
/// del monitor donde se encuentra, respetando un margen.
fn anclar_abajo_izquierda(win: &tauri::WebviewWindow) {
    if let (Ok(Some(monitor)), Ok(size)) = (win.current_monitor(), win.outer_size()) {
        let scr = monitor.size();
        let x = MARGEN_X;
        let y = (scr.height as i32) - (size.height as i32) - MARGEN_Y;
        let _ = win.set_position(tauri::PhysicalPosition::new(x, y.max(0)));
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let win = app
                .get_webview_window("mascota")
                .expect("falta la ventana 'mascota'");

            // Posición inicial: abajo a la izquierda.
            anclar_abajo_izquierda(&win);

            // Cargar ajustes (config.json en la carpeta de datos de la app).
            let config_dir = app.path().app_config_dir().unwrap_or_else(|_| ".".into());
            let cfg = settings::cargar(&config_dir);

            // --- Icono de bandeja con menú ---
            let pausar = MenuItem::with_id(app, "pausar", "Pausar / reanudar", true, None::<&str>)?;
            let esconder = MenuItem::with_id(app, "esconder", "Mostrar / esconder", true, None::<&str>)?;
            let salir = MenuItem::with_id(app, "salir", "Salir", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&esconder, &pausar, &salir])?;

            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .tooltip("Joaquincillo")
                .on_menu_event(|app, event| match event.id.as_ref() {
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
                    _ => {}
                })
                .build(app)?;

            // --- Monitores en segundo plano ---
            // Detección de actividad de teclado -> evento "mascota://on-fire".
            activity::iniciar_monitor(app.handle().clone(), cfg.on_fire_apm);

            // Sondeo del calendario (Microsoft Graph) -> evento "mascota://reunion".
            calendar::iniciar_monitor(app.handle().clone(), cfg.clone(), config_dir.clone());

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
        ])
        .on_window_event(|window, event| {
            // Al cerrar, solo escondemos (la app sigue viva en la bandeja).
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
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
