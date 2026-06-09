// Auto-actualización con Velopack.
//
// - Comprobación automática (cada INTERVALO_H horas): si hay versión nueva, SOLO avisa
//   en el bocadillo. No reinicia (para no interrumpir mientras trabajas).
// - `comprobar_ahora`: lo llama el botón "Buscar actualizaciones" de la bandeja. Ahí sí
//   descarga, aplica y reinicia, porque lo ha pedido el usuario.
//
// Requiere que la app se haya instalado con el instalador de Velopack (vpk). En modo
// `npm run dev` no hay nada que actualizar, así que esto no hará nada útil ahí.

use crate::settings::Settings;
use serde::Serialize;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use velopack::{sources::GithubSource, UpdateCheck, UpdateManager};

// Cada cuánto comprueba (en horas) la tarea de fondo.
const INTERVALO_H: u64 = 6;

/// Aviso de texto para el bocadillo.
#[derive(Clone, Serialize)]
struct Aviso {
    texto: String,
}

fn nuevo_um(cfg: &Settings) -> Result<UpdateManager, Box<dyn std::error::Error>> {
    let source = GithubSource::new(cfg.github_repo.trim(), None, false);
    Ok(UpdateManager::new(source, None, None)?)
}

/// Tarea de fondo: cada pocas horas, si hay versión nueva, avisa (sin reiniciar).
pub fn iniciar_monitor(app: AppHandle, cfg: Settings) {
    if cfg.github_repo.trim().is_empty() {
        return;
    }
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(15)); // no competir con el arranque
        loop {
            match nuevo_um(&cfg).and_then(|um| um.check_for_updates().map_err(Into::into)) {
                Ok(UpdateCheck::UpdateAvailable(_)) => {
                    let _ = app.emit("mascota://update-disponible", ());
                }
                Ok(_) => {}
                Err(e) => eprintln!("[updater] {e}"),
            }
            std::thread::sleep(Duration::from_secs(INTERVALO_H * 3600));
        }
    });
}

/// Comprobación bajo demanda (botón de la bandeja): si hay update, descarga y reinicia;
/// si no, avisa de que ya está al día.
pub fn comprobar_ahora(app: AppHandle, cfg: Settings) {
    if cfg.github_repo.trim().is_empty() {
        let _ = app.emit("mascota://aviso", Aviso { texto: "El auto-update no está configurado.".into() });
        return;
    }
    std::thread::spawn(move || {
        let resultado = (|| -> Result<bool, Box<dyn std::error::Error>> {
            let um = nuevo_um(&cfg)?;
            if let UpdateCheck::UpdateAvailable(updates) = um.check_for_updates()? {
                let _ = app.emit("mascota://update", ()); // "me actualizo y vuelvo"
                um.download_updates(&updates, None)?;
                std::thread::sleep(Duration::from_secs(5)); // que se vea el aviso
                um.apply_updates_and_restart(&updates)?;    // instala y reinicia
                Ok(true)
            } else {
                Ok(false)
            }
        })();

        match resultado {
            Ok(true) => {}
            Ok(false) => {
                let _ = app.emit("mascota://aviso", Aviso { texto: "✅ Ya tienes la última versión".into() });
            }
            Err(e) => {
                eprintln!("[updater] {e}");
                let _ = app.emit("mascota://aviso", Aviso { texto: "⚠️ No pude comprobar actualizaciones".into() });
            }
        }
    });
}
