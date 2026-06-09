// Auto-actualización con Velopack.
//
// Comprueba periódicamente si hay una versión nueva publicada en las Releases del
// repo de GitHub (config.json -> github_repo). Si la hay: avisa en el bocadillo,
// la descarga y reinicia para aplicarla.
//
// Requiere que la app se haya instalado con el instalador de Velopack (vpk). En modo
// `npm run dev` no hay nada que actualizar, así que esto no hará nada útil ahí.

use crate::settings::Settings;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use velopack::{sources::GithubSource, UpdateCheck, UpdateManager};

// Cada cuánto comprobar si hay actualización.
const INTERVALO_H: u64 = 6;

pub fn iniciar_monitor(app: AppHandle, cfg: Settings) {
    if cfg.github_repo.trim().is_empty() {
        return; // sin repo configurado, no hay auto-update
    }

    std::thread::spawn(move || {
        // Pequeña espera para no competir con el arranque.
        std::thread::sleep(Duration::from_secs(15));
        loop {
            if let Err(e) = comprobar(&app, &cfg) {
                eprintln!("[updater] {e}");
            }
            std::thread::sleep(Duration::from_secs(INTERVALO_H * 3600));
        }
    });
}

fn comprobar(app: &AppHandle, cfg: &Settings) -> Result<(), Box<dyn std::error::Error>> {
    let source = GithubSource::new(cfg.github_repo.trim(), None, false);
    let um = UpdateManager::new(source, None, None)?;

    if let UpdateCheck::UpdateAvailable(updates) = um.check_for_updates()? {
        // Avisar al usuario (Joaquincillo lo dice en su bocadillo).
        let _ = app.emit("mascota://update", ());
        // Descargar en segundo plano.
        um.download_updates(&updates, None)?;
        // Dar unos segundos para que se vea el aviso y luego aplicar + reiniciar.
        std::thread::sleep(Duration::from_secs(5));
        um.apply_updates_and_restart(&updates)?;
    }
    Ok(())
}
