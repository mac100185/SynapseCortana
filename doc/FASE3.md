# SynapseCortana — FASE 3: El Cuerpo Holográfico

## Descripción

FASE 3 del proyecto SynapseCortana: integrar un avatar 3D en una ventana separada, transparente y siempre visible (always-on-top), mientras la ventana de chat existente (FASE 1 y 2) permanece funcional. El avatar se muestra de frente al arrancar, con los materiales originales del modelo 3D, y reacciona a los estados del sistema (idle, listening, thinking, speaking) con movimientos naturales. El usuario controla la rotación manualmente y puede mover el avatar por la pantalla.

## Estado

✅ **FASE 3 completada.** Las cuatro subfases están implementadas y validadas:

- **FASE 3.1** — Refactor a dos ventanas (avatar chromeless + chat existente). ✅ **Completada**.
- **FASE 3.2** — Avatar 3D con Three.js (cargar `cortana_completa.glb`, materiales originales). ✅ **Completada**.
- **FASE 3.3** — Estados reactivos (idle, listening, thinking, speaking) con movimientos naturales. ✅ **Completada**.
- **FASE 3.4** — Interacción (click = dictar, triple-click = cerrar, rueda = girar, drag = mover). ✅ **Completada**.

## Objetivos de FASE 3

1. **Avatar 3D visible** en una ventana separada, transparente, sin bordes, always-on-top.
2. **Ventana de chat existente** (FASE 1+2) sigue funcionando sin cambios.
3. **Click izquierdo** en el avatar toggle del dictado por voz (inmediato, sin delay).
4. **Click derecho** en el avatar alterna la visibilidad de la ventana de chat.
5. **Triple-click** (3 clicks rápidos en <500ms) cierra el avatar.
6. **Rueda del mouse** gira el modelo manualmente.
7. **Arrastrar** (click + mover >10px) mueve la ventana por la pantalla.
8. **Estados reactivos**: el avatar cambia de movimiento según el estado del sistema.
9. **Sincronización entre ventanas**: el chat se entera cuando el dictado se inicia/detiene desde el avatar.
10. **Modelo 3D**: `cortana_completa.glb` (18k triángulos, materiales originales del Tripo3D, sin shader holográfico).

## Arquitectura de dos ventanas

```
┌─ Ventana 1: Avatar (chromeless + transparente + always-on-top) ─┐
│                                                                  │
│   ┌──────────────────┐                                           │
│   │   Cortana 3D    │   ← Three.js carga cortana_completa.glb  │
│   │  (materiales     │   ← Materiales originales (sin holograma) │
│   │   originales)    │   ← Iluminación realista (key+fill+rim)  │
│   └──────────────────┘                                           │
│                                                                  │
│   Click: toggle dictado (inmediato)                             │
│   3 clicks: cerrar avatar                                        │
│   Click-der: toggle ventana de chat                              │
│   Rueda: girar modelo                                            │
│   Arrastrar: mover ventana                                       │
│                                                                  │
└──────────────────────────────────────────────────────────────────┘

┌─ Ventana 2: Chat (la actual, sin cambios visuales) ─────────────┐
│   ⚙️ Configuración | 💬 Chat                                     │
│   ...                                                            │
│   Hola, soy Synapse Cortana.                                     │
│   [Mensajes, TTS, STT, todo igual que FASE 2.5]                │
│   ...                                                            │
│   El botón 🎙️ se sincroniza cuando el dictado se activa        │
│   desde el avatar (evento stt:state).                           │
└──────────────────────────────────────────────────────────────────┘
```

## Modelo 3D

### Origen

El modelo fue generado por **Tripo3D** a partir de la imagen de referencia `doc/Cortana2.png` (mujer joven con vestido, generada por IA). El `.glb` original del usuario pesaba 5.9 MB con ~192.000 triángulos.

### Procesamiento aplicado

1. **Importación** del `.glb` original (`Cortana2.glb`) en Blender 4.0.2.
2. **Re-escalado** a 1.75m de altura (estándar humanoide).
3. **Decimación** con modificador Decimate (ratio ~0.094) de 192k a ~18k triángulos.
4. **Materiales originales conservados**: NO se aplica shader holográfico. El modelo se muestra con los colores y texturas que Tripo3D generó desde la imagen de referencia.
5. **Smooth shading** aplicado.
6. **Exportación** a `frontend/assets/cortana_completa.glb` (835 KB).

### Especificaciones finales

| Atributo | Valor |
|----------|-------|
| Archivo | `frontend/assets/cortana_completa.glb` |
| Tamaño | 835 KB |
| Vértices | 16.134 |
| Triángulos | ~18.000 |
| Altura | 1.75m |
| Material | Original del Tripo3D (sin holograma) |
| Rigging | Ninguno (FASE 4) |
| Animaciones | Ninguna (procedural en Three.js) |

### Renderizado en Three.js

- **WebGLRenderer** con `alpha: true` (fondo transparente), `ACESFilmicToneMapping`.
- **Materiales**: se conservan los materiales originales del `.glb`. No se reemplazan por shaders custom.
- **Luces**:
  - `AmbientLight` blanca (0.5 intensidad).
  - `DirectionalLight` key blanca (1.2) frontal-superior-derecha.
  - `DirectionalLight` fill cálida (0.4) desde la izquierda.
  - `DirectionalLight` rim azulada (0.6) desde atrás.
- **Cámara**: `PerspectiveCamera` con FOV 30°, posición calculada automáticamente desde el bounding box del modelo para ver el cuerpo completo de pies a cabeza.
- **El modelo arranca de frente** (`rotation.y = 0`), sin rotación automática.

### Scripts de Blender

- `tools/analyze_glb.py` — analiza un `.glb` (verts, tris, materiales, bounding box).
- `tools/export_cortana2.py` — decima + re-escala + exporta con materiales originales.
- `tools/export_original_materials.py` — versión alternativa del procesamiento.
- `tools/setup_sculpt_scene.py` — prepara escena para esculpir manualmente.
- `tools/export_sculpted_model.py` — exporta modelo esculpido a `.glb`.
- `tools/prepare_user_glb.py` — decima + material holográfico + re-escala (versión anterior con shader).
- `tools/ollama_to_blendermcp.py` — puente Ollama ↔ BlenderMCP (agente con visión minimax-m3).

## FASE 3.1 — Refactor a dos ventanas

### Objetivo

Separar la aplicación en dos ventanas Tauri independientes.

### Cambios

#### `src-tauri/tauri.conf.json`

Dos ventanas + CSP desactivado + security config:

```json
{
  "app": {
    "withGlobalTauri": true,
    "security": { "csp": null },
    "windows": [
      {
        "label": "chat",
        "title": "Synapse Cortana - Chat",
        "width": 500, "height": 700,
        "resizable": true, "center": true,
        "url": "index.html"
      },
      {
        "label": "avatar",
        "title": "Synapse Cortana - Avatar",
        "width": 400, "height": 600,
        "decorations": false,
        "transparent": true,
        "alwaysOnTop": true,
        "resizable": false,
        "skipTaskbar": true,
        "url": "avatar.html"
      }
    ]
  }
}
```

#### `src-tauri/capabilities/default.json`

Actualizado de `"windows": ["main"]` a `"windows": ["chat", "avatar"]` para que ambas ventanas tengan permisos IPC.

#### `frontend/avatar.html` (nuevo)

HTML con canvas + spinner de carga + indicador de estado + error handler global.

#### `frontend/avatar.js` (nuevo)

Lógica del avatar con Three.js (~420 líneas). Ver detalle en FASE 3.2-3.4.

#### `frontend/vendor/` (nuevo)

Three.js r170 descargado localmente (sin CDN):
- `vendor/three.module.js` — Three.js core.
- `vendor/GLTFLoader.js` — cargador GLTF (imports parcheados a rutas relativas).
- `frontend/utils/BufferGeometryUtils.js` — dependencia del GLTFLoader (imports parcheados).

Los imports de `GLTFLoader.js` y `BufferGeometryUtils.js` fueron parcheados para usar rutas relativas en vez de bare imports (`'three'`), ya que los import maps no son soportados en WebKitGTK 4.1.

#### Comandos Tauri nuevos (en `lib.rs`)

- `toggle_chat_window(app) -> Result<bool, String>` — alterna visibilidad de la ventana del chat.
- `show_chat_window(app) -> Result<(), String>` — muestra la ventana del chat y **siempre la enfoca** (`set_focus`). Usado por el auto-send del backend cuando `autoSend` está inactivo.
- `show_avatar_window(app) -> Result<(), String>` — muestra la ventana del avatar si está oculta.
- `resize_avatar_window(width, height, app) -> Result<(), String>` — redimensiona la ventana del avatar.
- `set_avatar_state(state: String, app: AppHandle) -> Result<(), String>` — cambia estado del avatar + emite evento `avatar_state_change` a la ventana del avatar.
- `get_avatar_state() -> String` — devuelve el estado actual.
- `start_dragging(app) -> Result<(), String>` — inicia arrastre nativo de la ventana del avatar.
- `close_avatar_window(app) -> Result<(), String>` — **detiene STT** + **cierra ambas ventanas** (avatar y chat) + `app.exit(0)`. Triple-click.

#### `close_avatar_window` actualizado

Antes solo ocultaba el avatar. Ahora cierra toda la app de forma limpia:

```rust
#[tauri::command]
fn close_avatar_window(app: AppHandle) -> Result<(), String> {
    // Detener STT si está activo (evita thread colgado).
    let _ = stt_stop(app.clone());
    // Cerrar ambas ventanas.
    if let Some(w) = app.get_webview_window("avatar") { let _ = w.close(); }
    if let Some(w) = app.get_webview_window("chat")   { let _ = w.close(); }
    // Terminar el proceso.
    app.exit(0);
    Ok(())
}
```

#### `show_chat_window` siempre enfoca

```rust
#[tauri::command]
fn show_chat_window(app: AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("chat") {
        let _ = w.show();
        let _ = w.set_focus();
    }
    Ok(())
}
```

#### `on_window_event` intercepta `CloseRequested`

Es **crítico** distinguir el cierre del chat (X) del cierre del avatar (SO). Si el chat se destruye, `get_webview_window("chat")` retorna `None` y el auto-send falla.

```rust
.on_window_event(|window, event| {
    if let WindowEvent::CloseRequested { api, .. } = event {
        match window.label() {
            "chat"   => { api.prevent_close(); let _ = window.hide(); } // ocultar, NO destruir
            "avatar" => { window.app_handle().exit(0); }                // cerrar toda la app
            _        => {}
        }
    }
})
```

- **Chat (X)**: `api.prevent_close()` + `window.hide()` — la ventana se oculta, no se destruye. Sigue accesible vía `get_webview_window("chat")`.
- **Avatar (cierre por SO)**: `app.exit(0)` — cierra toda la app.

#### Auto-send desde el backend

`stt_stop` ahora **espera al thread del STT** (`handle.join()`) para tener la transcripción lista. Esto bloquea ~1.5s, pero garantiza el texto antes de continuar. Una `static LAST_TRANSCRIPTION` pasa el texto del thread al `stt_stop`.

Flujo dentro de `stt_stop`:

1. Hace `join()` del thread del STT (espera transcripción).
2. Lee `LAST_TRANSCRIPTION`.
3. Si hay texto, hace `chat.eval(...)` para inyectar el texto en el input + `sendMessage()` directamente desde el backend.
4. **Si `autoSend` activo**: NO muestra el chat (el mensaje ya se envió).
5. **Si `autoSend` inactivo**: llama `show_chat_window` para que el usuario revise el texto antes de enviar.

Por consiguiente, el listener `stt:final` en `app.js` **ya NO llama `sendMessage()`** (evita envío duplicado: el backend ya lo hizo).

#### Sincronización entre ventanas

El backend emite eventos `stt:state` cuando el STT arranca/detiene, para que ambas ventanas se sincronicen:

```rust
// En stt_start (backend):
let _ = app.emit("stt:state", json!({"recording": true}));
let _ = set_avatar_state("listening", app);

// En stt_stop (backend):
let _ = app.emit("stt:state", json!({"recording": false}));
let _ = set_avatar_state("idle", app);
```

El chat (`app.js`) escucha `stt:state` en `setupSttListeners()` y actualiza el botón mic cuando el dictado se inicia desde el avatar.

#### STT lee el modelo de settings (fix idioma)

`stt_start` ahora lee `stt_model_id` de `AppSettings` cuando no se pasa un `model_id` explícito. Antes usaba `DEFAULT_STT_MODEL_ID` (streaming inglés) ignorando el modelo configurado en settings (ej: Whisper base para español).

### Validación E2E de FASE 3.1

1. Arrancar el binario → dos ventanas aparecen (avatar + chat).
2. La ventana del chat funciona igual que antes (enviar mensaje, TTS, STT).
3. Click derecho en el avatar → la ventana del chat se oculta/muestra.
4. La ventana del avatar es siempre visible (always-on-top).
5. La ventana del avatar es transparente (se ve el escritorio detrás).
6. Click en el avatar → el botón mic del chat se actualiza a ⏹️ (sincronización).

## FASE 3.2 — Avatar 3D con Three.js

### Objetivo

Cargar `cortana_completa.glb` en la ventana del avatar con Three.js, usando los materiales originales del modelo (sin shader holográfico).

### Cambios

- **Three.js r170** local en `frontend/vendor/` (sin CDN).
- `frontend/avatar.js`:
  - `WebGLRenderer` con `alpha: true` (fondo transparente).
  - `GLTFLoader` para cargar `assets/cortana_completa.glb`.
  - Materiales originales conservados (NO reemplazados por shader custom).
  - `ACESFilmicToneMapping` para colores naturales.
  - Luces: ambient blanca + key blanca + fill cálida + rim azulada.
  - Cámara auto-encuadrada desde el bounding box para ver el cuerpo completo.
  - El modelo arranca de frente (`rotation.y = 0`).
  - Render loop a 60 fps.

### Cámara auto-encuadrada

Después de cargar el modelo, se calcula el bounding box y se posiciona la cámara automáticamente:

```javascript
const modelHeight = scaledSize.y;
const distance = modelHeight / (2 * Math.tan((camera.fov * Math.PI) / 180 / 2));
camera.position.set(0, modelHeight / 2, distance * 1.15);
camera.lookAt(0, modelHeight / 2, 0);
```

Esto asegura que TODO el cuerpo (de pies a cabeza) sea visible sin cortes.

### Validación E2E de FASE 3.2

1. El modelo se ve en la ventana del avatar (no pantalla en blanco).
2. Los colores son los originales del Tripo3D (no azul holográfico).
3. El cuerpo completo es visible (de pies a cabeza, sin cortes).
4. El framerate es estable (~60 fps).

## FASE 3.3 — Estados reactivos con movimientos naturales

### Objetivo

El avatar cambia de movimiento según el estado del sistema. Los movimientos son naturales (respiración, balanceo, inclinación) en vez de una rotación circular monótona.

### Estados

| Estado | Trigger | Movimiento |
|--------|---------|------------|
| **idle** | Sin actividad | Respiración suave (sube/baja 1.5cm), balanceo lateral (~1°), oscilación Y sutil (~5°), head tilt ocasional |
| **listening** | `stt_start` activo | Quieta, atenta, leve inclinación hacia adelante, micro-vibración, luz más intensa |
| **thinking** | `chat_and_speak` esperando LLM | Más erguida, mirando hacia arriba, oscilación Y más amplia (~9°), luz más tenue |
| **speaking** | TTS reproduciendo | Sway más marcado, respiración más rápida, gestos laterales ocasionales, sway Y suave |

### Rotación manual del usuario

El usuario controla la rotación Y del modelo con la rueda del mouse. La rotación del usuario (`userRotationY`) se combina con el sway sutil del estado:

```javascript
model.rotation.y = userRotationY + swayY;
```

No hay rotación automática circular. El avatar se queda quieto mirando al usuario, con solo movimientos sutiles.

### Implementación

- `avatar.js` escucha eventos `avatar_state_change` desde el backend.
- El backend `stt_start`/`stt_stop` llama automáticamente `set_avatar_state("listening"/"idle")`.
- El frontend `app.js` emite `"thinking"` al iniciar `chat_and_speak` y `"speaking"` al reproducir TTS.
- Las animaciones son procedurales (sin rigging): `position.y`, `rotation.x/y/z`, e intensidad de luces.

### Estados en español + colores

El indicador de estado del avatar muestra texto en español y cambia de color según el estado, con transición suave (0.3s):

| Estado | Texto | Color | CSS class |
|--------|-------|-------|-----------|
| `idle` | En espera | Gris | `state-idle` |
| `listening` | Escuchando | Amarillo | `state-listening` |
| `thinking` | Pensando | Azul claro | `state-thinking` |
| `speaking` | Hablando | Verde claro | `state-speaking` |

```css
.state-indicator { transition: color 0.3s ease; }
.state-idle      { color: gray; }
.state-listening { color: #e0c30a; }
.state-thinking  { color: #6fa8dc; }
.state-speaking  { color: #93c47d; }
```

## FASE 3.4 — Interacción

### Objetivo

El usuario interactúa con el avatar mediante clicks, rueda y arrastre.

### Comportamiento

| Acción | Cómo | Resultado | Latencia |
|--------|------|-----------|----------|
| **Click izquierdo** | Solo primer click de la ráfaga (sin mover >10px) | Toggle dictado (inmediato) | 0ms |
| **Triple-click** | 3 clicks rápidos (<600ms) | Cierra avatar + chat + app (`app.exit(0)`) | Inmediato |
| **Click derecho** | Detectado en `mousedown` (no `contextmenu`) | Toggle ventana de chat | Inmediato |
| **Arrastrar** | Click + mover >10px | Mueve ventana por la pantalla | Nativo del SO |
| **Rueda** | Scroll up/down | Gira el modelo manualmente | Inmediato |
| **Menú contextual** | `window.addEventListener("contextmenu", e => e.preventDefault())` | Suprimido (no aparece menú del navegador) | — |

### Lógica de clicks

El click simple ejecuta el toggle de dictado **inmediatamente** (sin delay), pero solo en el **primer click de la ráfaga** (para no togglear 3 veces durante un triple-click). Se lleva un contador de clicks para detectar el triple-click (<600ms):

```javascript
canvas.addEventListener("click", (e) => {
  if (!invoke || didDrag) return;
  clickCount++;
  if (clickCount >= 3) {
    // Triple-click: cerrar avatar + chat + app.
    invoke("close_avatar_window");
    return;
  }
  // Solo el primer click de la ráfaga dispara el toggle.
  if (clickCount === 1) {
    if (avatarState === "listening") {
      invoke("stt_stop");
    } else {
      invoke("stt_start", { modelId: null });
    }
  }
  // Resetear contador después de 600ms.
  clickTimer = setTimeout(() => { clickCount = 0; }, 600);
});
```

El click derecho se detecta en `mousedown` (no `contextmenu`) para respuesta inmediata:

```javascript
canvas.addEventListener("mousedown", (e) => {
  if (e.button === 2) invoke("toggle_chat_window");
});
```

El menú contextual del navegador se suprime globalmente para que no aparezca tras el click derecho:

```javascript
window.addEventListener("contextmenu", (e) => e.preventDefault());
```

El flag `didDrag` asegura que un arrastre no se interprete como click. El umbral de 10px distingue click de drag.

### Mover el avatar (drag)

El arrastre usa `start_dragging` (comando Tauri) que delega al gestor de ventanas nativo del SO:

```javascript
canvas.addEventListener("mousemove", (e) => {
  if (e.buttons & 1) {
    const dx = e.screenX - mouseDownPos.x;
    const dy = e.screenY - mouseDownPos.y;
    if (!didDrag && (Math.abs(dx) > 10 || Math.abs(dy) > 10)) {
      didDrag = true;
      invoke("start_dragging");
    }
  }
});
```

## Riesgos y mitigaciones

- **Rendimiento WebGL en WebView**: WebKitGTK puede ser lento. Mitigación: 18k triángulos es ligero, `powerPreference: 'low-power'`, sin post-processing.
- **Transparencia en Linux**: WebKitGTK soporta `transparent: true`. Mitigación: `alpha: true` en el renderer y CSS `background: transparent`.
- **Always-on-top en Wayland**: algunos compositores no respetan `alwaysOnTop`. Mitigación: usar X11.
- **Comunicación entre ventanas**: Tauri no permite `postMessage` directo. Mitigación: eventos Tauri (`app.emit`) + comando `set_avatar_state` + evento `stt:state`.
- **Carga del .glb**: 835 KB, parseo ~100ms. Mitigación: spinner de carga mientras carga.
- **Imports de Three.js**: los import maps no funcionan en WebKitGTK 4.1. Mitigación: parchear imports a rutas relativas en `GLTFLoader.js` y `BufferGeometryUtils.js`.
- **STT en inglés desde el avatar**: `stt_start` con `modelId: null` usaba el modelo inglés por defecto. Mitigación: ahora lee `stt_model_id` de settings (ej: Whisper base para español).
- **Sincronización del botón mic**: el chat no se enteraba cuando el dictado se iniciaba desde el avatar. Mitigación: backend emite `stt:state` a todas las ventanas; el chat escucha y actualiza el botón.
- **Clicks conflictivos**: el drag cancelaba clicks accidentales (umbral 5px muy bajo). Mitigación: umbral de 10px + flag `didDrag` + click inmediato sin delay.
- **Modelo sin rasgos faciales**: el modelo de Tripo3D tiene geometría pero los rasgos faciales son básicos. Mitigación: FASE 4 (normal maps, blendshapes, lip-sync).
- **Segfault al acceder al webview desde un thread separado**: el thread del STT no puede tocar `get_webview_window(...)` directamente (WebView no es `Send`/acceso seguro cross-thread). Mitigación: se movió `show` + `chat.eval(...)` a `stt_stop`, que corre en el hilo del comando Tauri, después de `handle.join()`.
- **Chat destruido al cerrar con la (X)**: si el usuario cierra el chat con la X de la ventana, `get_webview_window("chat")` retorna `None` y el auto-send del backend falla. Mitigación: `on_window_event` intercepta `CloseRequested` del chat con `api.prevent_close()` + `window.hide()` (la ventana se oculta, no se destruye).
- **Mensaje enviado dos veces**: el backend (auto-send) y el listener `stt:final` en `app.js` ambos llamaban `sendMessage()`, duplicando el envío. Mitigación: se removió `sendMessage()` del listener `stt:final`; el backend es el único que envía.
- **Menú contextual del navegador**: tras el click derecho aparecía el menú de WebKitGTK. Mitigación: `window.addEventListener("contextmenu", e => e.preventDefault())`.
- **App no termina al cerrar el avatar**: cerrar solo la ventana del avatar dejaba el proceso corriendo (chat invisible). Mitigación: `close_avatar_window` ahora detiene STT + cierra ambas ventanas + `app.exit(0)`; además `on_window_event` llama `app.exit(0)` en el `CloseRequested` del avatar.

## Herramientas de Blender (para referencia)

### BlenderMCP + Ollama

El puente `tools/ollama_to_blendermcp.py` permite controlar Blender desde Ollama:

```bash
# Modo agente (genera modelo desde prompt + imagen):
tools/.venv/bin/python tools/ollama_to_blendermcp.py "Crea una cabeza..."

# Modo MCP server (para Claude Desktop, Cursor, etc.):
tools/.venv/bin/python tools/ollama_to_blendermcp.py --mcp

# Inspeccionar escena de Blender:
tools/.venv/bin/python -c "
import sys; sys.path.insert(0, 'tools')
from ollama_to_blendermcp import BlenderMCPClient
print(BlenderMCPClient().get_scene_info())
"

# Procesar un .glb (decimar + re-escalar + exportar):
tools/.venv/bin/python -c "
import sys; sys.path.insert(0, 'tools')
from ollama_to_blendermcp import BlenderMCPClient
c = BlenderMCPClient()
code = open('tools/export_cortana2.py').read()
print(c.execute_code(code))
"
```

Modelo Ollama usado: `minimax-m3:cloud` (vision + tool-calling).

### Estructura de archivos de FASE 3

```
frontend/
├── avatar.html          # HTML del avatar (canvas + spinner + state indicator)
├── avatar.js            # Lógica Three.js (~420 líneas)
├── vendor/
│   ├── three.module.js  # Three.js r170 (1.3 MB)
│   └── GLTFLoader.js    # Cargador GLTF (imports parcheados)
├── utils/
│   └── BufferGeometryUtils.js  # Dependencia del GLTFLoader
└── assets/
    ├── Cortana.glb      # Modelo original del usuario (6 MB, 194k tris)
    ├── Cortana2.glb     # Modelo original v2 (5.9 MB, 192k tris)
    └── cortana_completa.glb  # Modelo procesado (835 KB, 18k tris) ← USAR ESTE

src-tauri/
├── tauri.conf.json      # Dos ventanas + CSP null
├── capabilities/default.json  # windows: ["chat", "avatar"]
└── src/lib.rs           # Comandos: toggle_chat_window, show_chat_window,
                          #          show_avatar_window, resize_avatar_window,
                          #          set_avatar_state, get_avatar_state,
                          #          start_dragging, close_avatar_window
                          #          + on_window_event (CloseRequested interceptado)

tools/
├── ollama_to_blendermcp.py  # Puente Ollama ↔ BlenderMCP
├── analyze_glb.py           # Analizador de .glb
├── export_cortana2.py       # Procesa Cortana2.glb
├── export_original_materials.py  # Exporta con materiales originales
├── setup_sculpt_scene.py    # Setup para esculpir manual
├── export_sculpted_model.py # Exporta modelo esculpido
├── prepare_user_glb.py      # Decima + material holográfico (versión anterior)
└── .venv/                   # Entorno Python con ollama + mcp
```

## Referencias

- Tauri 2 window config: https://v2.tauri.app/reference/config/#windowconfig
- Three.js GLTFLoader: https://threejs.org/docs/#examples/en/loaders/GLTFLoader
- Tauri events entre ventanas: https://v2.tauri.app/develop/calling-frontend/
- WebKitGTK transparent windows: https://webkitgtk.org/reference/webkit2gtk/stable/WebKitWebView.html
- Tauri start_dragging: https://v2.tauri.app/reference/javascript/api/namespacewindow/#startdragging

## Autor

Alan Mac-Arthur García Díaz — 2026