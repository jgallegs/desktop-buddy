# Joaquincillo 🧑‍💻

Mascota virtual de escritorio para Windows: el sprite de nuestro jefe flota en la
esquina inferior izquierda de la pantalla, hace animaciones, habla y reacciona a tu
jornada (cuando estás *on fire* tecleando, cuando se acerca una reunión de Teams/Outlook…).
Hecha con **Tauri** para que sea muy ligera y no caliente el portátil.

> 📐 Diseño técnico completo en [`PLAN.md`](./PLAN.md).

---

## Ver la mascota ahora mismo (sin instalar nada)

Abre **`preview.html`** con doble click. Verás a Joaquincillo animado y unos botones
para simular los eventos (hablar, on fire, reunión, pausa). Es solo una vista de prueba,
pero sirve para validar la animación y las frases sin montar el entorno.

---

## Ejecutar la app real (Tauri)

### Requisitos (una sola vez)

1. **Rust**: instálalo desde <https://rustup.rs>.
2. **Node.js** (18+): <https://nodejs.org>.
3. **WebView2**: ya viene en Windows 10/11 actualizados. Si no, se instala con
   [Evergreen Runtime](https://developer.microsoft.com/microsoft-edge/webview2/).
4. **Tauri CLI**:
   ```bash
   npm install
   ```

### Arrancar en modo desarrollo

```bash
npm run dev
```

La primera compilación de Rust tarda un poco; las siguientes son rápidas. Debería
aparecer Joaquincillo abajo a la izquierda. A los ~20 s saltará una **reunión de prueba**
(es la simulación de calendar.rs).

### Generar el instalador

```bash
npm run build
```

Produce un `.msi` (y un `.exe` NSIS) en `src-tauri/target/release/bundle/`.

---

## Publicar con GitHub Actions + Velopack (sin compilar en tu PC)

El workflow `.github/workflows/release.yml` compila Joaquincillo en los servidores de
GitHub (Windows) y publica un **instalador de Velopack con auto-actualización** en la
pestaña **Releases**. Así tus compañeros lo descargan sin instalar Rust ni nada.

**Una vez:** crea un repo en GitHub y sube esta carpeta (con Git o con GitHub Desktop).

**Para sacar una versión:**

- *Por etiqueta:* crea y sube una etiqueta de versión:
  ```bash
  git tag v0.1.0
  git push origin v0.1.0
  ```
  GitHub compila y publica la **Release `v0.1.0`** con el instalador.
- *Manual:* en GitHub → pestaña **Actions** → "Compilar y publicar Joaquincillo" →
  **Run workflow**, e introduce la versión (p. ej. `0.1.0`).

Tus compañeros entran a **Releases** y ejecutan el instalador (`Joaquincillo-...-Setup.exe`).

## Auto-actualización

Joaquincillo se actualiza solo con **Velopack**: cada pocas horas comprueba si hay una
versión nueva en las Releases del repo, y si la hay avisa en el bocadillo, la descarga y
se reinicia ya actualizado. Para que funcione, pon la URL del repo en `config.json`:

```json
"github_repo": "https://github.com/TU_USUARIO/joaquincillo"
```

Si lo dejas vacío, no busca actualizaciones. Solo funciona en la app **instalada** (no en
`npm run dev`). Cada nueva versión que publiques por Actions llega sola a todo el equipo.

> Nota: el instalador de Velopack no incluye WebView2; se asume presente (Win10/11
> actualizados ya lo traen).

---

## Cómo interactuar

- **Click en el sprite** → suelta una frase.
- **Icono de bandeja** (junto al reloj) → Mostrar/esconder, Pausar, Salir.
- Los clicks **fuera** del sprite atraviesan la ventana: puedes seguir trabajando con lo
  que haya debajo de la esquina.

---

## Personalizar

| Quiero… | Toca… |
|---------|-------|
| Cambiar las frases | `ui/src/speech.js` → objeto `FRASES` |
| Ajustar cuándo está "on fire" | `config.json` → `on_fire_apm` |
| Cambiar la antelación del aviso de reunión | `config.json` → `aviso_reunion_min` |
| Cambiar la hora del "fin de jornada" | `ui/src/states.js` → `HORA_FIN_JORNADA` |
| Cuánto tarda en dormirse | `ui/src/states.js` → `MS_PARA_DORMIR` |
| Añadir/mapear animaciones | `ui/src/sprite.js` → `ANIMACIONES`, y disparadores en `ui/src/states.js` |
| Mover la posición o el margen | `src-tauri/src/lib.rs` → `MARGEN_X` / `MARGEN_Y` |
| Tamaño de la ventana | `src-tauri/tauri.conf.json` → `width` / `height` (ajusta también la caja del sprite en `lib.rs`: `SPRITE_X0…Y1`) |

### Spritesheets

En `ui/assets/` hay 9 hojas, todas definidas en `ui/src/sprite.js` → `ANIMACIONES`:

| Animación | Archivo | Rejilla | Uso |
|-----------|---------|---------|-----|
| idle | `joaquincillo-idle.png` | 6×6 (36) | reposo |
| idle2 | `joaquincillo-idle2.png` | 6×6 (36) | reposo (variante) |
| idle3 | `joaquincillo-idle3.png` | 6×6 (36) | reposo (variante) |
| idle4 | `joaquincillo-idle4.png` | 6×6 (36) | reposo (variante) |
| talk | `joaquincillo-talk-v2.png` | 8×4 (32) | al hablar / saludar / avisar (recortada, sin la intro) |
| celebrar | `joaquincillo-celebrar.png` | 8×8 (64) | fin de jornada (18:00) |
| tumbarse | `joaquincillo-dormir-ciclo.png` | 8×8 (frames 0–40) | empezar a dormir |
| dormir | `joaquincillo-dormir-idle.png` | 8×8 (64) | durmiendo (en bucle) |
| levantarse | `joaquincillo-levantarse.png` | 8×8 (64) | despertar |

Los 4 idle entran en **rotación automática** (`IDLE_KEYS`): cada 8–16 s alterna a otra
variante para que no haga siempre lo mismo. La velocidad se bajó de 12 a **7 fps** para
que no parezca que tiembla (ajustable en cada entrada de `ANIMACIONES`).

Las hojas de pie llevan `cTop`/`cBot` (caja vertical del personaje) para que Joaquincillo
se vea **siempre del mismo tamaño**, con los pies en el mismo sitio. Las de dormir usan
`modo: "tumbado"` (escala fija `escala` + línea de suelo `piso`), porque el personaje está
horizontal y no se puede normalizar por altura.

### Secuencia de dormir

Tras **10 min sin actividad** (`MS_PARA_DORMIR` en `states.js`): se ensancha la ventana
de 280 a 380 px (la cama es ancha) → **tumbarse** (una vez) → **dormir** en bucle. Al
volver a usar el equipo (click sobre él, o teclear) → **levantarse** (una vez) → la ventana
vuelve a 280 px y sigue con el idle normal. El ensanchado lo hace el comando
`comando_set_dormido` en `lib.rs`; mientras duerme, toda la ventana es clicable para poder
despertarlo tocándolo.

> **Señalar (reunión):** no está hecha — el comportamiento de los eventos lo definiremos
> aparte. Ahora la reunión usa `talk` como placeholder.

---

## Ajustes (config.json)

Al arrancar por primera vez, la app crea **`config.json`** en
`%APPDATA%\com.equipo.joaquincillo\`. Plantilla en `config.example.json`:

```json
{
  "azure_client_id": "",        // ID de la app de Azure (vacío = calendario simulado)
  "azure_tenant": "common",     // o el ID del tenant corporativo
  "on_fire_apm": 280,           // pulsaciones/min para activar "on fire"
  "aviso_reunion_min": 5        // antelación del aviso de reunión
}
```

## Activar el calendario real (Microsoft Graph)

El login ya está implementado (flujo **device-code**, sin secretos). Solo falta darle el
client_id:

1. **Registra la app en Azure (Entra ID):**
   - Portal de Azure → *App registrations* → *New registration*.
   - En *Authentication* → *Advanced settings*, activa **"Allow public client flows"** (necesario para device-code).
   - En *API permissions* añade **`Calendars.Read`** (permiso delegado) y concede
     consentimiento. *(En empresa puede requerir aprobación de IT.)*
   - Copia el **Application (client) ID** (y el **Tenant ID** si no usas `common`).
2. Pega el client_id (y el tenant) en `config.json` y reinicia.
3. Al arrancar, Joaquincillo mostrará en su bocadillo una URL y un código: entra en la URL,
   mete el código y autoriza. El token se guarda y se renueva solo.

> **Plan B sin permisos de IT:** leer el calendario local de Outlook por COM/MAPI. Más
> frágil, pero no necesita registro en Azure. Documentado en `PLAN.md` §8.

---

## Novedades de Jira Cloud

Joaquincillo te avisa de novedades de Jira en su bocadillo: cambios en tus tareas,
cambios de estado, comentarios y menciones (y, si configuras un proyecto, también sus
novedades). Para activarlo:

1. Crea un **token de API**: <https://id.atlassian.com/manage-profile/security/api-tokens>
   → *Create API token* → copia el token.
2. Rellena en `config.json`:
   - `jira_site`: tu dominio, p. ej. `cexpress.atlassian.net` (sin `https://`).
   - `jira_email`: el email de tu cuenta de Atlassian.
   - `jira_token`: el token que acabas de crear.
   - `jira_proyecto`: *(opcional)* clave de un proyecto para seguir sus novedades, p. ej. `ABC`.
   - `jira_intervalo_s`: cada cuántos segundos mirar (60 por defecto; puedes bajar a 30).
3. Reinicia. A partir de ahí te irá avisando de lo nuevo.

> **Sobre el "tiempo real":** una app de escritorio no puede recibir *push* de Jira sin
> un servidor con URL pública (webhooks). Esto **sondea** cada `jira_intervalo_s`
> segundos, que se siente casi en directo sin que Jira te limite por exceso de llamadas.
> El token va en claro en `config.json`; para repartir conviene cifrarlo (ver `PLAN.md`).

---

## Privacidad

La detección de "on fire" usa un hook de teclado que **solo cuenta** pulsaciones de forma
agregada en memoria. No registra qué teclas se pulsan, no guarda nada en disco y no envía
nada a ningún servidor. Conviene explicarlo así al equipo. Detalle en `PLAN.md` §7.

El token de Microsoft se guarda en `%APPDATA%\com.equipo.joaquincillo\ms_token.json`.
Para una versión de reparto conviene cifrarlo con el almacén de credenciales de Windows
(pendiente, ver `PLAN.md` §8).

---

## Estructura

```
Joaquincillo/
├── PLAN.md            · diseño técnico
├── README.md          · este archivo
├── preview.html       · demo rápida (doble click)
├── package.json
├── config.example.json
├── ui/                · frontend (lo que se empaqueta en la app)
│   ├── index.html
│   ├── src/           · motor de sprites, estados, frases
│   └── assets/        · spritesheets
├── src-tauri/         · backend Rust (ventana, actividad, calendario, jira, updater)
└── .github/workflows/ · CI: compila y publica releases
```
