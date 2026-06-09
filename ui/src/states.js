// Máquina de estados de Joaquincillo.
//
// Decide, según los eventos que llegan, qué animación reproduce el sprite y qué
// frase muestra. Gestiona prioridades para que un aviso de reunión no lo pise
// una frase de relleno.

import { fraseAleatoria } from "./speech.js";
import { IDLE_KEYS } from "./sprite.js";

// Prioridad (mayor = manda). Un estado de menor prioridad no interrumpe a uno mayor.
// Jira tiene la MÁXIMA: interrumpe lo que haya, suelta su aviso y luego se reanuda
// lo interrumpido (p. ej. una reunión vuelve a salir). Nada se pierde: lo que no
// puede mostrarse ahora se encola y se reintenta al quedar libre.
const PRIORIDAD = {
  idle: 0,
  dormir: 1,
  on_fire: 2,
  talking: 3,
  celebrar: 4,
  reunion: 5,
  nervioso: 6, // reunión GO/NOGO
  jira: 7,     // novedades de Jira: lo más importante, interrumpe y luego reanuda
};

// Duraciones (ms).
const REUNION_MS = 60 * 1000; // cuánto se muestra el aviso de reunión
const AVISO_MS = 6 * 1000;    // cuánto se muestra un aviso de Jira
// Caducidad si un evento queda encolado (para no mostrar algo ya irrelevante).
const REUNION_CADUCA_MS = 15 * 60 * 1000;
const NERVIOSO_CADUCA_MS = 5 * 60 * 1000;
const AVISO_CADUCA_MS = 90 * 1000;

// Variación de idle: cada cuánto (ms) cambiar de hoja para no hacer siempre lo mismo.
const IDLE_CAMBIO_MIN = 8000;
const IDLE_CAMBIO_MAX = 16000;

// Tras este tiempo sin interacción ni teclear, Joaquincillo se duerme.
const MS_PARA_DORMIR = 10 * 60 * 1000; // 10 min

// Hora (0-23) a la que celebra el fin de la jornada (null = desactivado).
const HORA_FIN_JORNADA = 18;

// Detecta reuniones GO/NOGO por el título (varía mucho): "[CEX] - GO/NOGO",
// "[EVO] GO / NO GO MDA-6742", "GONOGO", "go/no-go"...
const RE_GONOGO = /go[\s\/]*no[\s\/-]*go/i;

// Cuánto se mantiene nervioso sin que se refresque el aviso (se renueva en cada sondeo).
const NERVIOSO_MS = 2 * 60 * 1000; // 2 min

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
    this._pendiente = null; // acción a ejecutar tras levantarse/calmarse
    this._nerviosoTimer = null;
    this._reunionTimer = null;
    this._avisoTimer = null;
    this._cola = [];        // eventos importantes que esperan turno (no se pierden)
    this._sostenida = null; // actividad en curso a reanudar si la interrumpen (reunión/nervios)
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
    if (this._nervios()) return this._calmar(() => this._hablarClick());
    this._hablarClick();
  }

  /** ¿Está en la secuencia de nervios (GO/NOGO)? */
  _nervios() {
    return this.estado === "nervioso" || this.estado === "calmando";
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
    if (this._durmiendo()) return this._despertar(() => this.reunion(datos));
    // Reunión GO/NOGO -> ponerse nervioso. El resto -> aviso normal.
    if (RE_GONOGO.test(datos.titulo || "")) return this._nervioso(datos);
    // Si ya hay una reunión en pantalla, refrescarla en sitio (no encolar otra).
    if (this.estado === "reunion") return this._pintarReunion(datos);
    this._pedir("reunion", () => this._pintarReunion(datos), REUNION_CADUCA_MS);
  }

  _pintarReunion(datos) {
    // Actividad sostenida: si algo la interrumpe (un Jira), se reanuda igual.
    this.estado = "reunion";
    this._sostenida = {
      nombre: "reunion",
      prio: PRIORIDAD.reunion,
      redisplay: () => this._pintarReunion(datos),
      caduca: 0,
    };
    this.sprite.play("talk");
    this.sprite.setSpeed(1.2);
    this.bocadillo.mostrar(`📅 "${datos.titulo}" en ${datos.minutos} min`, 0, true);
    clearTimeout(this._reunionTimer);
    this._reunionTimer = setTimeout(() => {
      this._sostenida = null;
      this.bocadillo.ocultar();
      this._volverAFondo();
    }, REUNION_MS);
  }

  /** Secuencia de nervios por reunión GO/NOGO: mira el reloj -> nervioso (bucle). */
  _nervioso(datos) {
    const texto = `😬 GO/NOGO: "${datos.titulo}" en ${datos.minutos} min`;
    // Si ya está nervioso, solo refresca el aviso y el temporizador (no reinicia la anim).
    if (this.estado === "nervioso") {
      this.bocadillo.mostrar(texto, 0);
      this._reprogramarNervioso();
      return;
    }
    this._pedir("nervioso", () => this._pintarNervioso(datos), NERVIOSO_CADUCA_MS);
  }

  _pintarNervioso(datos) {
    const texto = `😬 GO/NOGO: "${datos.titulo}" en ${datos.minutos} min`;
    this.estado = "nervioso";
    this._sostenida = {
      nombre: "nervioso",
      prio: PRIORIDAD.nervioso,
      redisplay: () => this._pintarNervioso(datos),
      caduca: 0,
    };
    this.sprite.setSpeed(1);
    this.bocadillo.mostrar(texto, 0); // persistente hasta que se calme
    this.sprite.play("reloj", { onComplete: () => {
      if (this.estado === "nervioso") this.sprite.play("nervioso");
    }});
    this._reprogramarNervioso();
  }

  _reprogramarNervioso() {
    clearTimeout(this._nerviosoTimer);
    this._nerviosoTimer = setTimeout(() => {
      if (this.estado === "nervioso") this._calmar();
    }, NERVIOSO_MS);
  }

  /**
   * Sale de los nervios reproduciendo "volver_normal" ENTERO y, solo al terminar,
   * ejecuta la acción pendiente (o vuelve al idle). Así nunca hay cortes raros.
   */
  _calmar(despues = null) {
    if (this.estado === "calmando") {
      if (despues) this._pendiente = despues;
      return;
    }
    clearTimeout(this._nerviosoTimer);
    this._sostenida = null; // ya no hay que reanudar el nervios
    this.estado = "calmando";
    this._pendiente = despues;
    this.bocadillo.ocultar();
    this.sprite.setSpeed(1);
    this.sprite.play("volver_normal", { onComplete: () => {
      this.estado = "idle";
      const accion = this._pendiente;
      this._pendiente = null;
      if (accion) accion();
      else this._volverAFondo();
    }});
  }

  /** Aviso de Jira (texto ya formateado por el backend). */
  jira(texto) {
    if (!texto) return;
    if (this._durmiendo()) return this._despertar(() => this.jira(texto));
    this._mostrarAviso(texto);
  }

  /** Pide mostrar un aviso de Jira (máxima prioridad: interrumpe lo que haya). */
  _mostrarAviso(texto) {
    this._pedir("jira", () => this._pintarAviso(texto), AVISO_CADUCA_MS);
  }

  _pintarAviso(texto) {
    this.estado = "jira";
    this.sprite.play("talk");
    this.bocadillo.mostrar(texto, AVISO_MS);
    clearTimeout(this._avisoTimer);
    // Al acabar, _volverAFondo reanuda lo interrumpido o muestra lo encolado.
    this._avisoTimer = setTimeout(() => this._volverAFondo(), AVISO_MS);
  }

  // ---- Cola de eventos importantes (nada se pierde) ------------------------

  /**
   * Intenta mostrar un evento. Si hay algo de prioridad ESTRICTAMENTE mayor en
   * pantalla, se encola y se reintenta cuando aquello acabe. Si este evento es
   * más prioritario, interrumpe lo que haya; y si lo interrumpido era una
   * actividad sostenida (una reunión), se reencola para reanudarla después.
   */
  _pedir(nombre, redisplay, caducaMs = 0) {
    const prio = PRIORIDAD[nombre] ?? 0;
    const prioActual = PRIORIDAD[this.estado] ?? 0;
    if (prio > prioActual) {
      clearTimeout(this._reunionTimer);
      clearTimeout(this._nerviosoTimer);
      clearTimeout(this._avisoTimer);
      if (this._sostenida) this._encolar(this._sostenida); // reanudar luego
      this._sostenida = null;
      this.estado = nombre;
      redisplay();
    } else {
      this._encolar({ nombre, prio, redisplay, caduca: caducaMs ? Date.now() + caducaMs : 0 });
    }
  }

  _encolar(entrada) {
    if (!entrada) return;
    // Para reunión/nervios solo guardamos una entrada (refrescamos la existente).
    const dup = this._cola.find((e) => e.nombre === entrada.nombre);
    if (dup && (entrada.nombre === "reunion" || entrada.nombre === "nervioso")) {
      dup.redisplay = entrada.redisplay;
      return;
    }
    this._cola.push(entrada);
    if (this._cola.length > 8) this._cola.shift();
  }

  /** Muestra el evento pendiente de mayor prioridad. Devuelve true si mostró algo. */
  _siguiente() {
    const ahora = Date.now();
    this._cola = this._cola.filter((e) => !e.caduca || e.caduca > ahora);
    if (this._cola.length === 0) return false;
    let idx = 0;
    for (let i = 1; i < this._cola.length; i++) {
      if (this._cola[i].prio > this._cola[idx].prio) idx = i;
    }
    const e = this._cola.splice(idx, 1)[0];
    this.estado = "idle"; // liberar para que el repintado se aplique
    this._sostenida = null;
    setTimeout(() => e.redisplay(), 200); // respiro para no pisar la animación
    return true;
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
    if (PRIORIDAD[nuevo] < (PRIORIDAD[this.estado] ?? 0)) return false;
    this.estado = nuevo;
    if (accion) accion();
    return true;
  }

  _volverAFondo() {
    // ¿Hay un evento importante esperando (reunión interrumpida, otro Jira…)?
    // Lo mostramos en vez de quedarnos en el fondo. Así nada se pierde.
    if (this._siguiente()) return;
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
