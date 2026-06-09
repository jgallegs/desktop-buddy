# Joaquincillo — Mascota virtual de escritorio

> App de escritorio para Windows: una mascota flotante (sprite de nuestro jefe) que
> vive en la esquina inferior izquierda, hace animaciones, habla, y reacciona a lo que
> pasa en tu jornada: cuando estás *on fire* trabajando, cuando se acerca una reunión
> en Teams/Outlook, etc. Pensada para ser **muy ligera** y no calentar el portátil.

---

## 1. Objetivos y principios

| Principio | Cómo lo cumplimos |
|-----------|-------------------|
| **Ligera** | Tauri (no Electron). El runtime usa el WebView2 que ya viene en Windows, sin empaquetar Chromium. RAM objetivo: 30–60 MB. CPU en reposo: ≈0%. |
| **Siempre visible, sin molestar** | Ventana sin bordes, transparente, *always-on-top*, anclada abajo a la izquierda, fuera de la barra de tareas. |
| **Reactiva** | Máquina de estados que cambia la animación y suelta frases según eventos (actividad de teclado, calendario, hora del día...). |
| **Conciliación / disuasión** | Tono amable y cómplice. Avisos útiles (reunión en 5 min), micro-ánimos cuando llevas un rato concentrado, recordatorio de pausas. |
| **Cero fricción** | Arranca con Windows, se esconde con un click, no roba el foco del teclado. |

El nombre de trabajo es **Joaquincillo**. Cámbialo libremente.

---

## 2. Stack técnico

- **Tauri v2** (Rust + WebView2) como contenedor de la app.
- **Frontend** en HTML/CSS/JS *vanilla* (sin frameworks → menos peso). El render del
  sprite se hace en un `<canvas>` con `requestAnimationFrame`.
- **Rust** para lo nativo: posicionado de ventana, hook de teclado/ratón para detectar
  actividad, y el cliente del calendario (Microsoft Graph).
- **Microsoft Graph API** para leer el calendario de Outlook/Teams (las reuniones de
  Teams aparecen como eventos normales en el calendario de Outlook).

### ¿Por qué Tauri y no Electron?

Electron empaqueta un Chromium completo → 150–200 MB de RAM por ventana, arranque lento.
Para algo que está **abierto todo el día en segundo plano**, eso es justo lo que no
queremos. Tauri reutiliza el WebView2 del sistema y el binario final pesa ~5–10 MB.

---

## 3. Arquitectura

```
┌──────────────────────────────────────────────────────────┐
│  Proceso Rust (src-tauri)                                  │
│                                                            │
│   ┌─────────────┐   ┌──────────────┐   ┌───────────────┐  │
│   │ activity.rs │   │ calendar.rs  │   │  main.rs      │  │
│   │ hook de     │   │ poll Graph   │   │ ventana,      │  │
│   │ teclado →   │   │ cada 60s →   │   │ tray, comandos│  │
│   │ APM/min     │   │ próximos     │   │               │  │
│   └──────┬──────┘   │ eventos      │   └───────┬───────┘  │
│          │          └──────┬───────┘           │          │
│          └─────────────────┴───────────────────┘          │
│                    emit("mascota://evento", …)             │
└────────────────────────────┬───────────────────────────────┘
                             │ eventos Tauri (IPC)
┌────────────────────────────▼───────────────────────────────┐
│  Frontend (WebView2)                                         │
│                                                              │
│   states.js  ──►  sprite.js  ──►  <canvas>                   │
│   (máquina       (anima el       (dibuja el frame)           │
│    de estados)    spritesheet)                               │
│        │                                                     │
│        └──►  speech.js  (bocadillos de diálogo)              │
└──────────────────────────────────────────────────────────────┘
```

El frontend nunca calcula nada pesado: solo escucha eventos del backend y decide qué
animación y qué frase mostrar. El backend hace el trabajo de bajo nivel y duerme entre
sondeos.

---

## 4. La ventana flotante

Configurada en `tauri.conf.json`:

- `transparent: true`, `decorations: false` → sin marco, fondo transparente.
- `alwaysOnTop: true` → siempre por encima de las demás ventanas.
- `skipTaskbar: true` → no aparece en la barra de tareas.
- `resizable: false`, `shadow: false`.
- Tamaño: ~280×420 px (suficiente para el sprite de 268×540 escalado + un bocadillo
  encima). Posición calculada en `main.rs` para anclarla abajo a la izquierda con un
  margen, leyendo el tamaño del monitor.

**Interacción sin robar foco:** el WebView solo captura el ratón cuando pasa por encima
del sprite o del bocadillo; el resto de la ventana es transparente y "click-through"
(`set_ignore_cursor_events`). Así puedes seguir trabajando con lo que hay debajo.

- **Click sobre el sprite** → frase aleatoria / interacción.
- **Click derecho / icono de bandeja** → menú: pausar, esconder, ajustes, salir.
- **Arrastrar** → reposicionar (se guarda la posición).

---

## 5. Motor de sprites (`sprite.js`)

El spritesheet actual: `assets/joaquincillo-idle.png`, **1608×3240 px, rejilla 6×6 = 36
frames**, cada frame **268×540 px**.

El motor es genérico: se le pasa una *config* por animación y reproduce los frames.

```js
const ANIMACIONES = {
  idle:    { sheet: "joaquincillo-idle.png", cols: 6, rows: 6, frames: 36, fps: 12, loop: true },
  // futuras: hablar, celebrar, asustado, dormir, señalar...
  // talk:  { sheet: "joaquincillo-talk.png", ... },
};
```

Cuando tengáis más spritesheets (hablar, celebrar...), solo se añaden aquí. Mientras
tanto, todos los estados reutilizan `idle` (o un subrango de frames).

---

## 6. Máquina de estados (`states.js`)

| Estado | Disparador | Animación | Comportamiento |
|--------|-----------|-----------|----------------|
| `idle` | por defecto | idle suave | Frases ocasionales cada X min. |
| `on_fire` | APM (teclas/min) por encima de umbral durante N seg | idle rápido / "celebrar" | "¡Estás imparable! 🔥" — no interrumpe. |
| `meeting_soon` | evento de calendario en < 5 min | "señalar" / agitado | Bocadillo persistente: "Reunión *X* en 5 min". |
| `idle_largo` | sin actividad > 30 min | "dormir" | Sugerencia de pausa o se duerme. |
| `talking` | click del usuario | "hablar" | Frase aleatoria del repertorio. |
| `saludo` | arranque / cambio de franja horaria | idle | "Buenos días", "casi es la hora de comer"... |

Prioridad: `meeting_soon` > `talking` > `on_fire` > `idle_largo` > `idle`.

---

## 7. Detección de actividad (`activity.rs`)

Hook global de teclado en Windows (`SetWindowsHookEx` con `WH_KEYBOARD_LL`) que **solo
cuenta** pulsaciones — nunca registra qué teclas (no es un keylogger, no almacena nada).
Cada segundo calcula las pulsaciones del último minuto (APM) y, si cruza un umbral,
emite `mascota://on-fire`. Coste despreciable.

> Privacidad: el contador es en memoria, agregado, y no se persiste ni se envía a ningún
> sitio. Conviene documentarlo bien para tranquilidad del equipo.

Alternativa aún más ligera si el hook da problemas con el antivirus corporativo:
`GetLastInputInfo` (solo distingue activo/inactivo, sin contar el ritmo).

---

## 8. Integración con el calendario (`calendar.rs`)

Microsoft Graph, endpoint `GET /me/calendarView?startDateTime=…&endDateTime=…`.

1. **Registro de la app en Azure (Entra ID).** Crear una app registration, permiso
   delegado `Calendars.Read`, flujo de autenticación **device code** o **PKCE** (no
   requiere secreto en cliente, ideal para una app de escritorio).
2. El backend sondea el calendario cada 60 s, busca el próximo evento y, cuando faltan
   ≤ 5 min, emite `mascota://reunion` con título y minutos restantes.
3. El token se guarda cifrado en el almacén de credenciales de Windows.

> En muchos entornos corporativos hace falta que **IT apruebe** la app registration.
> Plan B sin permisos: leer el calendario local de Outlook por COM/MAPI. Más frágil,
> pero no necesita aprobación de IT. Lo dejamos como opción documentada.

---

## 9. Rendimiento

- WebView2 ya está en Windows → no se empaqueta navegador.
- `requestAnimationFrame` se **pausa** cuando la ventana no es visible.
- El canvas se redibuja solo cuando cambia el frame (12 fps, no 60).
- Sondeos del backend espaciados (calendario 60 s, actividad agregada por segundo).
- Sin dependencias JS pesadas.

Objetivo realista: **30–60 MB RAM, ~0% CPU en reposo**.

---

## 10. Roadmap

**Fase 0 — Prototipo (este entregable)**
Ventana flotante transparente + sprite idle animado abajo a la izquierda + bocadillos +
máquina de estados + *stubs* listos de actividad y calendario.

**Fase 1 — Reactividad real**
Activar el hook de teclado (on_fire) y la integración de Graph (meeting_soon) con tokens
reales.

**Fase 2 — Más personalidad**
Spritesheets adicionales (hablar, celebrar, dormir, señalar), repertorio de frases más
rico, franjas horarias, "modo no molestar".

**Fase 3 — Pulido y reparto**
Icono de bandeja, ajustes, arranque con Windows, instalador `.msi` firmado, ajustes por
usuario.

---

## 11. Estructura del proyecto

```
Joaquincillo/
├── PLAN.md                 ← este documento
├── README.md               ← cómo arrancar
├── package.json            ← scripts de dev/build (Tauri CLI)
├── index.html              ← shell del frontend
├── assets/
│   └── joaquincillo-idle.png   ← spritesheet (36 frames, 6×6)
├── src/                    ← frontend
│   ├── styles.css
│   ├── sprite.js           ← motor de animación
│   ├── states.js           ← máquina de estados
│   ├── speech.js           ← bocadillos + repertorio de frases
│   └── main.js             ← orquesta todo, escucha eventos del backend
└── src-tauri/             ← backend Rust
    ├── Cargo.toml
    ├── build.rs
    ├── tauri.conf.json
    └── src/
        ├── main.rs         ← ventana, posición, tray, comandos
        ├── activity.rs     ← detección de actividad (on_fire)
        └── calendar.rs     ← cliente de Microsoft Graph
```
