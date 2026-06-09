// Ajustes de Joaquincillo, leídos de config.json en la carpeta de datos de la app
// (%APPDATA%\com.equipo.joaquincillo\config.json en Windows).
//
// Si el archivo no existe, se crea con valores por defecto la primera vez, para que
// el usuario solo tenga que abrirlo y rellenar el client_id de Azure.

use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Settings {
    /// Application (client) ID de la app registrada en Azure (Entra ID).
    /// Si está vacío, el calendario corre en modo SIMULACIÓN.
    #[serde(default)]
    pub azure_client_id: String,

    /// Tenant de Azure. "common" vale para la mayoría; o el ID del tenant corporativo.
    #[serde(default = "tenant_por_defecto")]
    pub azure_tenant: String,

    /// Pulsaciones por minuto a partir de las cuales se considera "on fire".
    #[serde(default = "apm_por_defecto")]
    pub on_fire_apm: u64,

    /// Minutos de antelación para avisar de una reunión.
    #[serde(default = "aviso_por_defecto")]
    pub aviso_reunion_min: i64,

    // --- Jira Cloud ---
    /// Dominio del sitio, p. ej. "cexpress.atlassian.net" (sin https://).
    #[serde(default)]
    pub jira_site: String,
    /// Email de tu cuenta de Atlassian.
    #[serde(default)]
    pub jira_email: String,
    /// Token de API (id.atlassian.com -> Security -> API tokens).
    #[serde(default)]
    pub jira_token: String,
    /// (Opcional) Clave de un proyecto para avisar también de sus novedades, p. ej. "ABC".
    #[serde(default)]
    pub jira_proyecto: String,
    /// Cada cuántos segundos sondear Jira (mínimo práctico ~30).
    #[serde(default = "jira_intervalo_por_defecto")]
    pub jira_intervalo_s: u64,

    /// Repo de GitHub para auto-actualización con Velopack. Por defecto el oficial;
    /// se puede sobreescribir en config.json. Vacío = sin auto-update.
    #[serde(default = "repo_por_defecto")]
    pub github_repo: String,
}

fn tenant_por_defecto() -> String { "common".into() }
fn apm_por_defecto() -> u64 { 280 }
fn aviso_por_defecto() -> i64 { 5 }
fn jira_intervalo_por_defecto() -> u64 { 60 }
fn repo_por_defecto() -> String { "https://github.com/jgallegs/desktop-buddy".into() }

impl Default for Settings {
    fn default() -> Self {
        Settings {
            azure_client_id: String::new(),
            azure_tenant: tenant_por_defecto(),
            on_fire_apm: apm_por_defecto(),
            aviso_reunion_min: aviso_por_defecto(),
            jira_site: String::new(),
            jira_email: String::new(),
            jira_token: String::new(),
            jira_proyecto: String::new(),
            jira_intervalo_s: jira_intervalo_por_defecto(),
            github_repo: repo_por_defecto(),
        }
    }
}

impl Settings {
    /// ¿Hay credenciales de Azure configuradas? Si no, el calendario va en simulación.
    pub fn calendario_configurado(&self) -> bool {
        !self.azure_client_id.trim().is_empty()
    }

    /// ¿Hay credenciales de Jira configuradas?
    pub fn jira_configurado(&self) -> bool {
        !self.jira_site.trim().is_empty()
            && !self.jira_email.trim().is_empty()
            && !self.jira_token.trim().is_empty()
    }
}

/// Carga config.json del directorio dado; si no existe, lo crea con valores por defecto.
pub fn cargar(config_dir: &Path) -> Settings {
    let _ = std::fs::create_dir_all(config_dir);
    let path = config_dir.join("config.json");

    if let Ok(txt) = std::fs::read_to_string(&path) {
        match serde_json::from_str::<Settings>(&txt) {
            Ok(s) => return s,
            Err(e) => eprintln!("[settings] config.json inválido ({e}); uso valores por defecto"),
        }
    }

    // No existe (o estaba corrupto): escribir plantilla por defecto.
    let s = Settings::default();
    if let Ok(txt) = serde_json::to_string_pretty(&s) {
        let _ = std::fs::write(&path, txt);
    }
    s
}

/// Mueve los secretos que pudieran estar en texto plano en config.json al llavero
/// (Administrador de credenciales de Windows) y los borra del archivo. Después deja
/// en memoria el valor efectivo leído del llavero, para que el resto del código lo use
/// igual que antes.
pub fn migrar_secretos(config_dir: &Path, s: &mut Settings) {
    let token_en_archivo = s.jira_token.trim().to_string();
    if !token_en_archivo.is_empty() {
        // Guardar en el llavero y borrar del archivo (reescribir sin el token).
        if crate::secrets::guardar("jira_token", &token_en_archivo).is_ok() {
            let mut disco = s.clone();
            disco.jira_token = String::new();
            if let Ok(txt) = serde_json::to_string_pretty(&disco) {
                let _ = std::fs::write(config_dir.join("config.json"), txt);
            }
        }
    }
    // Token efectivo desde el llavero (si existe) -> memoria.
    if let Some(t) = crate::secrets::obtener("jira_token") {
        s.jira_token = t;
    }
}
