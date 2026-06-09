// Máquina de estados de Joaquincillo.
//
// Decide, según los eventos que llegan, qué animación reproduce el sprite y qué
// frase muestra. Gestiona prioridades para que un aviso de reunión no lo pise
// una frase de relleno.

import { fraseAleatoria } from "./speech.js";
import { IDLE_KEYS } from "./sprite.js";

// Prioridad (mayor = manda). Un estado de menor prioridad no interrumpe a uno mayor.
const PRIORIDAD = {
  idle: 0,
  dormir: 1,
  on_fire: 2,
  talking: 3,
  celebrar: 4,
  reunion: 5,
};

// Variación de idle: cada cuánto (ms) cambiar de hoja para no hacer siempre lo mismo.
const IDLE_CAMBIO_MIN = 8000;
const IDLE_CAMBIO_MAX = 16000;

// Tras este tiempo sin interacción ni teclear, Joaquincillo se duerme.
const MS_PARA_DORMIR = 10 * 60 * 1000; // 10 min

// Hora (0-23) a la que celebra el fin de la jornada (null = desactivado).
const HORA_FIN_JORNADA = 18;

export class MaquinaEstados {
  /**
   * @param sprite  motor de sprites
   * @param bocadillo  bocadillo de diálogo
   * @param onDormir  callback(bool): avisa al backend para ensanchar/estrechar la
   *                  ventana al dormir/despertar (la cama es ancha).
   */
  constructor(sprite, bocadillo, onDormir = () => {}) {
    this.sprite = sprite;
    this.bocadillo = bocadillo;
    this.onDormir = onDormir;
    this.estado = "idle";
    this.onFireActivo = false;
    this.ultimaInteraccion = Date.now();
    this.idleActual = IDLE_KEYS[0];
    this.idleTimer = null;
    this.celebradoEl = null;
    this._pendiente = null; // acción a ejecutar tras levantarse
    this._aplicar("idle");
    this._programarCambioIdle();
  }

  // ---- Entradas de eventos -------------------------------------------------

  /** ¿Está dormido o en mitad de tumbarse/levantarse? */
  _durmiendo() {
    return this.estado === "dormir" || this.estado === "despertando";
  }

  /** Click del usuario sobre el sprite. */
  click() {
    this.ultimaInteraccion = Date.now();
    if (this._durmiendo()) return this._despertar(() => this._hablarClick());
    this._hablarClick();
  }

  _hablarClick() {
    this._transicion("talking", () => {
      this.bocadillo.mostrar(fraseAleatoria("click"), 3500);
      this.sprite.play("talk");
      setTimeout(() => this._volverAFondo(), 3500);
    });
  }

  /** Cualquier actividad de teclado (despierta de la siesta). */
  actividad() {
    this.ultimaInteraccion = Date.now();
    if (this._durmiendo()) this._despertar();
  }

  /** Cambio en el estado "on fire" (tecleando mucho). */
  onFire(activo) {
    this.onFireActivo = activo;
    if (activo) {
      this.ultimaInteraccion = Date.now();
      if (this._durmiendo()) { this._despertar(); return; }
      this._transicion("on_fire", () => {
        this.sprite.setSpeed(1.6);
        this.bocadillo.mostrar(fraseAleatoria("on_fire"), 3500);
      });
    } else if (this.estado === "on_fire") {
      this._volverAFondo();
    }
  }

  /** Reunión próxima (payload de calendar.rs: { titulo, minutos }). */
  reunion(datos) {
    if (this._durmiendo()) return this._despertar(() => this._mostrarReunion(datos));
    this._mostrarReunion(datos);
  }

  /** Aviso de Jira (texto ya formateado por el backend). */
  jira(texto) {
    if (!texto) return;
    if (this._durmiendo()) return this._despertar(() => this._mostrarAviso(texto));
    this._mostrarAviso(texto);
  }

  /** Aviso breve: habla y muestra el texto unos segundos, luego vuelve al idle. */
  _mostrarAviso(texto) {
    this._transicion("talking", () => {
      this.sprite.play("talk");
      this.bocadillo.mostrar(texto, 6000);
      setTimeout(() => this._volverAFondo(), 6000);
    });
  }

  _mostrarReunion({ titulo, minutos }) {
    this._transicion("reunion", () => {
      this.sprite.play("talk"); // (comportamiento de eventos pendiente de rediseñar)
      this.sprite.setSpeed(1.2);
      const txt = `📅 "${titulo}" en ${minutos} min`;
      this.bocadillo.mostrar(txt, 0, true);
      setTimeout(() => {
        this.bocadillo.ocultar();
        this._volverAFondo();
      }, 60000);
    });
  }

  /** Tick periódico (lo llama main.js cada ~1 s): dormir, fin de jornada. */
  tick() {
    if (HORA_FIN_JORNADA != null) {
      const ahora = new Date();
      const hoy = ahora.toISOString().slice(0, 10);
      if (ahora.getHours() === HORA_FIN_JORNADA && this.celebradoEl !== hoy && this.estado !== "reunion") {
        this.celebradoEl = hoy;
        this._transicion("celebrar", () => {
          this.sprite.play("celebrar");
          this.sprite.setSpeed(1);
          this.bocadillo.mostrar(fraseAleatoria("fin_jornada"), 6000);
          setTimeout(() => this._volverAFondo(), 6000);
        });
        return;
      }
    }

    const inactivoMs = Date.now() - this.ultimaInteraccion;
    if (inactivoMs > MS_PARA_DORMIR && this.estado === "idle") {
      this._dormir();
    }
  }

  /** Saludo según la hora (lo llama main.js al arrancar). */
  saludar() {
    const h = new Date().getHours();
    let cat = "saludo_tarde";
    if (h < 12) cat = "saludo_manana";
    else if (h >= 14 && h < 16) cat = "saludo_comida";
    this.sprite.play("talk");
    this.bocadillo.mostrar(fraseAleatoria(cat), 4000);
    setTimeout(() => this._volverAFondo(), 4000);
  }

  // ---- Secuencia de dormir -------------------------------------------------

  _dormir() {
    this.estado = "dormir";
    this.sprite.setSpeed(1);
    this.onDormir(true); // ensanchar la ventana (la cama es ancha)
    this.bocadillo.mostrar(fraseAleatoria("dormir"), 4000);
    this.sprite.play("tumbarse", { onComplete: () => {
      if (this.estado === "dormir") this.sprite.play("dormir");
    }});
  }

  /**
   * Despierta: reproduce "levantarse" entero y SOLO al terminar estrecha la ventana
   * y ejecuta la acción pendiente (hablar, avisar...). Nunca se corta a la mitad.
   */
  _despertar(despues = null) {
    if (this.estado === "despertando") {
      if (despues) this._pendiente = despues;
      return;
    }
    this.estado = "despertando";
    this._pendiente = despues;
    this.bocadillo.ocultar();
    this.sprite.play("levantarse", { onComplete: () => {
      this.onDormir(false); // volver a la ventana estrecha
      this.estado = "idle";
      const accion = this._pendiente;
      this._pendiente = null;
      if (accion) accion();
      else this._volverAFondo();
    }});
  }

  // ---- Variación de idle ---------------------------------------------------

  _programarCambioIdle() {
    clearTimeout(this.idleTimer);
    const t = IDLE_CAMBIO_MIN + Math.random() * (IDLE_CAMBIO_MAX - IDLE_CAMBIO_MIN);
    this.idleTimer = setTimeout(() => {
      if (this.estado === "idle") {
        this.idleActual = this._otroIdle();
        this.sprite.play(this.idleActual);
      }
      this._programarCambioIdle();
    }, t);
  }

  _otroIdle() {
    if (IDLE_KEYS.length <= 1) return IDLE_KEYS[0];
    let k;
    do {
      k = IDLE_KEYS[Math.floor(Math.random() * IDLE_KEYS.length)];
    } while (k === this.idleActual);
    return k;
  }

  // ---- Interno -------------------------------------------------------------

  _transicion(nuevo, accion) {
    if (PRIORIDAD[nuevo] < (PRIORIDAD[this.estado] ?? 0)) return;
    this.estado = nuevo;
    if (accion) accion();
  }

  _volverAFondo() {
    const fondo = this.onFireActivo ? "on_fire" : "idle";
    this.estado = fondo;
    this.sprite.setSpeed(fondo === "on_fire" ? 1.6 : 1);
    if (fondo === "idle") {
      this.idleActual = this._otroIdle();
      this.sprite.play(this.idleActual);
    } else {
      this._aplicar("on_fire");
    }
  }

  _aplicar(estado) {
    if (estado === "idle") this.sprite.play(this.idleActual);
    else this.sprite.play("idle");
  }
}
