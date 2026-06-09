// Lógica del panel de ajustes. Va en archivo aparte (no inline) por la CSP.

const invoke = window.__TAURI__?.core?.invoke;
const $ = (id) => document.getElementById(id);
const CAMPOS = ["jira_site", "jira_email", "jira_proyecto", "jira_intervalo_s",
  "azure_client_id", "azure_tenant", "aviso_reunion_min", "on_fire_apm", "github_repo"];

// Cargar los valores actuales en el formulario.
async function cargar() {
  if (!invoke) return;
  try {
    const a = await invoke("cargar_ajustes");
    for (const k of CAMPOS) {
      if (a[k] !== undefined && a[k] !== null) $(k).value = a[k];
    }
    if (a.jira_token_set) {
      $("jira_token").placeholder = "•••••••• (guardado — vacío = no cambiar)";
    }
  } catch (e) {
    console.error(e);
  }
}

// Guardar.
$("form").addEventListener("submit", async (ev) => {
  ev.preventDefault();
  if (!invoke) return;
  const datos = {
    jira_site: $("jira_site").value.trim(),
    jira_email: $("jira_email").value.trim(),
    jira_token: $("jira_token").value, // vacío = no cambiar
    jira_proyecto: $("jira_proyecto").value.trim(),
    jira_intervalo_s: Number($("jira_intervalo_s").value) || 60,
    azure_client_id: $("azure_client_id").value.trim(),
    azure_tenant: $("azure_tenant").value.trim() || "common",
    aviso_reunion_min: Number($("aviso_reunion_min").value) || 5,
    on_fire_apm: Number($("on_fire_apm").value) || 280,
    github_repo: $("github_repo").value.trim(),
  };
  try {
    await invoke("guardar_ajustes", { datos });
    $("ok").classList.add("ver"); // el backend cierra la ventana; por si acaso
  } catch (e) {
    alert("No se pudo guardar: " + e);
  }
});

$("cancelar").addEventListener("click", () => {
  window.__TAURI__?.window?.getCurrentWindow?.().close();
});

cargar();
