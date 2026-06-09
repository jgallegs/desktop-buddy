// Motor de animación de spritesheets.
//
// Genérico: se le da una tabla de animaciones (cada una con su PNG, rejilla y fps)
// y reproduce los frames sobre un <canvas>. El render se pausa cuando la ventana no
// está visible y solo redibuja al cambiar de frame -> consumo mínimo.

// cTop/cBot = caja del personaje (px verticales dentro del frame, sin contar el
// relleno transparente). Sirve para que Joaquincillo se vea SIEMPRE del mismo tamaño
// y con los pies en el mismo sitio, aunque cada hoja tenga distinto margen.
export const ANIMACIONES = {
  idle: {
    sheet: "assets/joaquincillo-idle.png",
    cols: 6, rows: 6, frames: 36,
    fps: 7,        // antes 12 -> bajado para que no "tiemble"
    loop: true, cTop: 8, cBot: 531,
  },

  idle2: { sheet: "assets/joaquincillo-idle2.png", cols: 6, rows: 6, frames: 36, fps: 7, loop: true, cTop: 1, cBot: 524 },
  idle3: { sheet: "assets/joaquincillo-idle3.png", cols: 6, rows: 6, frames: 36, fps: 7, loop: true, cTop: 6, cBot: 529 },
  idle4: { sheet: "assets/joaquincillo-idle4.png", cols: 6, rows: 6, frames: 36, fps: 7, loop: true, cTop: 8, cBot: 531 },

  // Hablar: hoja recortada (sin la intro). 8x4 = 32 frames. La usa el estado "talking".
  // escala 0.99: la hoja de hablar está dibujada un pelín más grande; esto la iguala al idle.
  talk:  { sheet: "assets/joaquincillo-talk-v2.png", cols: 8, rows: 4, frames: 32, fps: 14, loop: true, cTop: 1, cBot: 598, escala: 0.99 },

  celebrar: { sheet: "assets/joaquincillo-celebrar.png", cols: 8, rows: 8, frames: 64, fps: 14, loop: true, cTop: 11, cBot: 532, fallback: "talk" },

  // --- Secuencia de dormir (personaje tumbado en una cama) ---
  // modo "tumbado": no se normaliza por altura; se escala por `escala` (factor fijo,
  // elegido para que el cuerpo tumbado mida igual en las 3 piezas) y se ancla la línea
  // del suelo `piso` (y en px del frame) al borde inferior del lienzo.
  tumbarse:  { sheet: "assets/joaquincillo-dormir-ciclo.png", cols: 8, rows: 8, frames: 64, desde: 0, hasta: 40, fps: 18, loop: false, modo: "tumbado", escala: 0.747, piso: 544, fallback: "idle" },
  dormir:    { sheet: "assets/joaquincillo-dormir-idle.png",  cols: 8, rows: 8, frames: 64, fps: 6,  loop: true,  modo: "tumbado", escala: 0.67,  piso: 381, fallback: "idle" },
  levantarse:{ sheet: "assets/joaquincillo-levantarse.png",   cols: 8, rows: 8, frames: 64, fps: 18, loop: false, modo: "tumbado", escala: 0.80,  piso: 475, fallback: "idle" },
};

// Altura objetivo del personaje (de pie) en el lienzo (px).
const ALTURA_PERSONAJE = 360;

// Qué animaciones cuentan como "idle" (rotación automática).
export const IDLE_KEYS = ["idle", "idle2", "idle3", "idle4"];

export class SpriteEngine {
  constructor(canvas) {
    this.canvas = canvas;
    this.ctx = canvas.getContext("2d");
    this.ctx.imageSmoothingEnabled = false; // pixel art nítido
    this.cache = new Map(); // sheet -> HTMLImageElement | null (null = no existe)
    this.anim = null;
    this.frame = 0;
    this.desde = 0;
    this.hasta = 0;
    this.acc = 0;
    this.last = 0;
    this.speed = 1;
    this.paused = false;
    this.raf = null;
    this.onComplete = null;

    document.addEventListener("visibilitychange", () => {
      if (document.hidden) this.detener();
      else this.arrancar();
    });
  }

  _cargar(sheet) {
    if (this.cache.has(sheet)) {
      const v = this.cache.get(sheet);
      return v ? Promise.resolve(v) : Promise.reject(new Error("hoja no disponible"));
    }
    return new Promise((resolve, reject) => {
      const img = new Image();
      img.onload = () => { this.cache.set(sheet, img); resolve(img); };
      img.onerror = () => { this.cache.set(sheet, null); reject(new Error("no se pudo cargar " + sheet)); };
      img.src = sheet;
    });
  }

  /**
   * Reproduce una animación por nombre. Si la hoja no existe, cae a su `fallback`.
   * opts.onComplete: se llama cuando una animación NO-loop llega al último frame.
   */
  async play(nombre, opts = {}) {
    let cfg = ANIMACIONES[nombre] || ANIMACIONES.idle;
    let clave = nombre;
    try {
      await this._cargar(cfg.sheet);
    } catch {
      clave = cfg.fallback || "idle";
      cfg = ANIMACIONES[clave] || ANIMACIONES.idle;
      try { await this._cargar(cfg.sheet); } catch { return; }
    }
    this.anim = { nombre: clave, ...cfg };
    this.desde = cfg.desde ?? 0;
    this.hasta = cfg.hasta ?? (cfg.frames - 1);
    this.frame = this.desde;
    this.acc = 0;
    this.onComplete = opts.onComplete || null;
  }

  setSpeed(mult) { this.speed = mult; }

  /** Ajusta el tamaño del lienzo (al ensanchar/estrechar la ventana al dormir).
   *  Si el tamaño no cambia, no hace nada: cambiar canvas.width borra el lienzo, y
   *  el WebView puede emitir eventos 'resize' espurios que causarían parpadeo. */
  ajustarLienzo(w, h) {
    w = Math.round(w);
    h = Math.round(h);
    if (this.canvas.width === w && this.canvas.height === h) return;
    this.canvas.width = w;
    this.canvas.height = h;
    this.ctx.imageSmoothingEnabled = false; // cambiar width resetea el contexto
    this._dibujar();
  }

  /** Precarga todas las hojas para que cambiar de animación nunca deje un hueco en blanco. */
  precargarTodo() {
    for (const k of Object.keys(ANIMACIONES)) {
      this._cargar(ANIMACIONES[k].sheet).catch(() => {});
    }
  }

  arrancar() {
    if (this.raf || this.paused) return;
    this.last = performance.now();
    const loop = (t) => { this._tick(t); this.raf = requestAnimationFrame(loop); };
    this.raf = requestAnimationFrame(loop);
  }

  detener() {
    if (this.raf) cancelAnimationFrame(this.raf);
    this.raf = null;
  }

  setPausa(p) {
    this.paused = p;
    if (p) this.detener();
    else this.arrancar();
  }

  _tick(t) {
    if (!this.anim) return;
    const dt = (t - this.last) / 1000;
    this.last = t;

    const dur = 1 / (this.anim.fps * this.speed);
    this.acc += dt;
    let cambiado = false;
    while (this.acc >= dur) {
      this.acc -= dur;
      this.frame++;
      if (this.frame > this.hasta) {
        if (this.anim.loop) {
          this.frame = this.desde;
        } else {
          this.frame = this.hasta;
          if (this.onComplete) {
            const cb = this.onComplete;
            this.onComplete = null;
            cb();
          }
        }
      }
      cambiado = true;
    }
    if (cambiado) this._dibujar();
  }

  _dibujar() {
    const a = this.anim;
    if (!a) return;
    const img = this.cache.get(a.sheet);
    if (!img) return;

    const fw = img.width / a.cols;
    const fh = img.height / a.rows;
    const sx = (this.frame % a.cols) * fw;
    const sy = Math.floor(this.frame / a.cols) * fh;

    const { width: cw, height: ch } = this.canvas;
    this.ctx.clearRect(0, 0, cw, ch);

    let escala, dy;
    if (a.modo === "tumbado") {
      escala = a.escala ?? 1;
      const piso = a.piso ?? fh;
      dy = ch - piso * escala;
    } else {
      const cTop = a.cTop ?? 0;
      const cBot = a.cBot ?? fh;
      escala = (ALTURA_PERSONAJE / (cBot - cTop)) * (a.escala ?? 1);
      dy = ch - cBot * escala;
    }
    const dw = fw * escala;
    const dh = fh * escala;
    const dx = (cw - dw) / 2;

    this.ctx.drawImage(img, sx, sy, fw, fh, dx, dy, dw, dh);
  }
}
