// Orquestador del frontend: arranca el motor de sprites, conecta la máquina de
// estados y escucha los eventos que emite el backend (Rust).

import { SpriteEngine } from "./sprite.js";
import { Bocadillo } from "./speech.js";
import { MaquinaEstados } from "./states.js";

// API de Tauri expuesta globalmente (withGlobalTauri: true). Si abrimos el
// frontend en un navegador normal (sin Tauri), estos quedan como no-ops para
// poder probar la animación de forma aislada.
const TAURI = window.__TAURI__;
const listen = TAURI?.event?.listen ?? (async () => () => {});
const invoke = TAURI?.core?.invoke ?? (async () => {});

const ALTO_BOCADILLO = 60; // px reservados arriba para el bocadillo

async function arrancar() {
  const canvas = document.getElementById("lienzo");
  const bocadilloEl = document.getElementById("bocadillo");
  const bocadilloTexto = document.getElementById("bocadillo-texto");

  const sprite = new SpriteEngine(canvas);
  const bocadillo = new Bocadillo(bocadilloEl, bocadilloTexto);

  // El lienzo ocupa el ancho de la ventana (que se ensancha al dormir).
  const ajustar = () =>
    sprite.ajustarLienzo(window.innerWidth, Math.max(200, window.innerHeight - ALTO_BOCADILLO));
  ajustar();
  window.addEventListener("resize", ajustar);

  // Al dormir/despertar, pedimos al backend ensanchar/estrechar la ventana.
  const onDormir = (dormido) => invoke("comando_set_dormido", { dormido });
  const estados = new MaquinaEstados(sprite, bocadillo, onDormir);

  await sprite.play("idle");
  sprite.arrancar();

  // Saludo inicial según la hora.
  setTimeout(() => estados.saludar(), 800);

  // Click sobre el sprite -> habla (o despierta si duerme).
  canvas.addEventListener("click", () => estados.click());

  // --- Eventos del backend ---
  await listen("mascota://on-fire", (e) => estados.onFire(e.payload === true));
  await listen("mascota://actividad", () => estados.actividad());
  await listen("mascota://reunion", (e) => estados.reunion(e.payload));
  await listen("mascota://jira", (e) => estados.jira(e.payload?.texto || ""));
  await listen("mascota://update", () => estados.jira("✨ ¡Nueva versión! Me actualizo y vuelvo 🔄"));
  await listen("mascota://login", (e) => {
    const { codigo, url } = e.payload || {};
    sprite.play("talk");
    bocadillo.mostrar(`Para ver tu calendario, entra en ${url} y pon el código: ${codigo}`, 0);
  });
  await listen("mascota://login-ok", () => bocadillo.mostrar("¡Conectado a tu calendario! 📅", 4000));
  await listen("mascota://toggle-pausa", () => sprite.setPausa(!sprite.paused));

  // Tick ambiental cada segundo (dormir, fin de jornada...).
  setInterval(() => estados.tick(), 1000);
}

window.addEventListener("DOMContentLoaded", arrancar);
