// Almacén seguro de credenciales.
//
// Usa el Administrador de credenciales de Windows (vía la crate `keyring`, que por
// debajo usa DPAPI). Los tokens quedan cifrados y atados a tu cuenta de Windows;
// no se guardan en texto plano en ningún archivo.

use keyring::Entry;

const SERVICIO: &str = "Joaquincillo";

/// Lee un secreto del llavero. Devuelve None si no existe o hay error.
pub fn obtener(clave: &str) -> Option<String> {
    Entry::new(SERVICIO, clave).ok()?.get_password().ok()
}

/// Guarda (o reemplaza) un secreto en el llavero.
pub fn guardar(clave: &str, valor: &str) -> Result<(), Box<dyn std::error::Error>> {
    Entry::new(SERVICIO, clave)?.set_password(valor)?;
    Ok(())
}

/// Borra un secreto del llavero (ignora si no existía).
#[allow(dead_code)]
pub fn borrar(clave: &str) {
    if let Ok(e) = Entry::new(SERVICIO, clave) {
        let _ = e.delete_credential();
    }
}
