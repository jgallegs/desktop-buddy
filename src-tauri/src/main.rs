// Evita abrir una consola extra en Windows en modo release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Velopack DEBE ser lo primero: en instalación/actualización el ejecutable se
    // lanza con argumentos especiales y esta llamada los gestiona (y puede salir/
    // reiniciar el proceso) antes de arrancar la app.
    velopack::VelopackApp::build().run();

    joaquincillo_lib::run();
}
