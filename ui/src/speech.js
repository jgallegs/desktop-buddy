// Bocadillos de diálogo + repertorio de frases de Joaquincillo.

// Frases por categoría. Edita y amplía libremente — aquí vive la personalidad.
export const FRASES = {
  // Al hacer click en el sprite.
  click: [
    "¿Todo bien por ahí?",
    "Recuerda: el café también es trabajar.",
    "Vas estupendamente, sigue así.",
    "¿Una pausita de 30 segundos? Yo te cubro.",
    "Si necesitas algo, aquí estoy flotando.",
    "Ojo, que no se diga que no avisé de las reuniones.",
  ],
  // Saludos según la franja horaria.
  saludo_manana: ["¡Buenos días! A por el día ☕", "Arrancamos. Tú puedes."],
  saludo_tarde: ["¿Qué tal la mañana?", "Recta final de la tarde, ánimo."],
  saludo_comida: ["Casi es la hora de comer 🍽️", "No te saltes la comida, ¿eh?"],
  // Cuando detecta que estás muy concentrado tecleando.
  on_fire: [
    "¡Estás imparable! 🔥",
    "Menudo ritmo llevas hoy.",
    "Así da gusto. No te quemes igualmente.",
  ],
  // Cuando se va a dormir por inactividad.
  dormir: [
    "Me echo una siestecita... 😴",
    "Si has salido, aquí vigilo.",
    "Zzz... avísame cuando vuelvas.",
  ],
  // Fin de la jornada.
  fin_jornada: [
    "¡Buen trabajo hoy! A descansar 🎉",
    "Fin de jornada. Desconecta, te lo has ganado.",
    "Lo de hoy ha estado genial. ¡Hasta mañana!",
  ],
};

export function fraseAleatoria(categoria) {
  const lista = FRASES[categoria] || FRASES.click;
  return lista[Math.floor(Math.random() * lista.length)];
}

export class Bocadillo {
  constructor(el, textoEl) {
    this.el = el;
    this.textoEl = textoEl;
    this.timer = null;
  }

  /**
   * Muestra una frase. `ms` = cuánto permanece (0 = persistente hasta ocultar).
   * `aviso` = estilo de alerta (reunión).
   */
  mostrar(texto, ms = 4000, aviso = false) {
    clearTimeout(this.timer);
    this.textoEl.textContent = texto;
    this.el.classList.toggle("aviso", aviso);
    this.el.classList.remove("oculto");
    if (ms > 0) {
      this.timer = setTimeout(() => this.ocultar(), ms);
    }
  }

  ocultar() {
    this.el.classList.add("oculto");
  }
}
