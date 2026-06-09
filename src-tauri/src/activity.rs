// Detección de actividad de teclado — estado "on_fire".
//
// QUÉ HACE: instala un hook global de teclado de bajo nivel en Windows que
// SOLO CUENTA pulsaciones. No registra qué teclas se pulsan, no almacena nada,
// no envía nada a ningún sitio. Es un contador agregado en memoria.
//
// CÓMO FUNCIONA: cada segundo se calcula cuántas pulsaciones hubo en los últimos
// 60 s (APM, "actions per minute"). Si supera el umbral (config.json -> on_fire_apm),
// se emite el evento `mascota://on-fire` con `true`; si baja, con `false`.
//
// COSTE: despreciable. El hook solo incrementa un contador atómico.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Emitter};

// Contador global de pulsaciones (lo incrementa el hook de Windows).
static PULSACIONES: AtomicU64 = AtomicU64::new(0);

/// Lanza el monitor de actividad en un hilo aparte. `umbral_apm` viene de settings.
pub fn iniciar_monitor(app: AppHandle, umbral_apm: u64) {
    // Hilo que instala el hook de teclado y corre su bucle de mensajes (solo Windows).
    #[cfg(windows)]
    std::thread::spawn(|| unsafe { instalar_hook_windows() });

    // Hilo que cada segundo evalúa el APM y emite el estado on_fire.
    std::thread::spawn(move || {
        // Buffer circular de las pulsaciones de los últimos 60 segundos.
        let mut ventana = [0u64; 60];
        let mut idx = 0usize;
        let mut ultimo_total = 0u64;
        let mut on_fire = false;
        let mut seg_desde_actividad = 99u32; // para no spamear el evento de actividad

        loop {
            std::thread::sleep(Duration::from_secs(1));

            let total = PULSACIONES.load(Ordering::Relaxed);
            let delta = total.saturating_sub(ultimo_total);
            ultimo_total = total;

            // Señal de actividad (para despertar de la siesta), como mucho cada 3 s.
            seg_desde_actividad = seg_desde_actividad.saturating_add(1);
            if delta > 0 && seg_desde_actividad >= 3 {
                seg_desde_actividad = 0;
                let _ = app.emit("mascota://actividad", ());
            }

            ventana[idx] = delta;
            idx = (idx + 1) % ventana.len();

            let apm: u64 = ventana.iter().sum();
            let nuevo = apm >= umbral_apm;

            if nuevo != on_fire {
                on_fire = nuevo;
                let _ = app.emit("mascota://on-fire", on_fire);
            }
        }
    });
}

#[cfg(windows)]
unsafe fn instalar_hook_windows() {
    use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, DispatchMessageW, GetMessageW, SetWindowsHookExW, TranslateMessage, HHOOK,
        MSG, WH_KEYBOARD_LL, WM_KEYDOWN, WM_SYSKEYDOWN,
    };

    // Callback del hook: solo nos interesa contar los KEYDOWN.
    unsafe extern "system" fn cb(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code >= 0 {
            let msg = wparam.0 as u32;
            if msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN {
                PULSACIONES.fetch_add(1, Ordering::Relaxed);
            }
        }
        CallNextHookEx(HHOOK::default(), code, wparam, lparam)
    }

    // hMod = NULL es válido para un hook de bajo nivel cuyo proc está en este .exe.
    use windows::Win32::Foundation::HINSTANCE;
    let _hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(cb), HINSTANCE::default(), 0);

    // Un hook de bajo nivel necesita un bucle de mensajes vivo en este hilo.
    let mut msg = MSG::default();
    while GetMessageW(&mut msg, None, 0, 0).as_bool() {
        let _ = TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }
}
