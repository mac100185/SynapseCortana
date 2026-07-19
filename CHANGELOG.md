# Changelog

Todos los cambios notables de SynapseCortana se documentan en este archivo.

## [0.1.0] — 2026-06-29

### FASE 1: El Cascarón Conectivo ✅

- Conexión WebSocket con OpenClaw gateway v4
- Handshake con identidad Ed25519 persistente (`~/.config/synapse-cortana/device.key`)
- Autenticación con token compartido (modo `webchat-ui/ui`)
- Comando RPC `chat.send` con `sessionKey` configurable
- Modo CLI `--cli-test-handshake` para testing sin GUI
- Validación E2E contra gateway OpenClaw v2026.6.6/v2026.6.8

### FASE 2: La Voz de Cortana ✅

#### FASE 2.1 — Integración sherpa-onnx
- Motor TTS `sherpa-onnx` 1.13 embebido en el binario
- Voz por defecto: `es_AR-daniela-high` (femenina argentina, 114 MB)
- Catálogo de 5 voces en español (3 masculinas + 2 femeninas)
- Descarga perezosa de voces desde GitHub Releases (k2-fsa)
- Caché en `~/.config/synapse-cortana/voices/`
- Modo CLI `--cli-test-speak` para testing TTS sin GUI

#### FASE 2.2 — Comandos Tauri
- `tts_list_voices`, `tts_status`, `tts_set_voice`, `tts_synthesize`
- Flujo end-to-end: frontend → backend → TTS → WAV base64 → audio HTML

#### FASE 2.3 — Integración con OpenClaw
- Comando `chat_and_speak`: envía mensaje al LLM + sintetiza TTS en una sola llamada
- Cambio de `client.id = "gateway-client"/"backend"` a `"webchat-ui"/"ui"`
- Streaming de respuestas del LLM con acumulación de chunks
- Dedetección de fin de respuesta por silencio (configurable) o `chat.done`

#### FASE 2.4 — Mejoras de UX
- **FASE 2.4.A**: Pestañas Config/Chat, persistencia de settings, contraste en `<select>`, densidad visual
- **FASE 2.4.B**: Selector dinámico de `sessionKey` vía `sessions.list` del gateway
- **FASE 2.4.C**: STT con `sherpa-onnx` (Whisper medium + Whisper base; el streaming Zipformer se dejó de empaquetar y el default pasó a Whisper medium para dictado en español fiable)
- **FASE 2.4.D**: Selector de micrófono (`stt_list_microphones`, `stt_set_microphone`)

#### FASE 2.5 — Rendimiento
- Pre-carga de voz TTS al iniciar la app (background thread)
- Caché TTS persistente en disco (SHA-256 del texto → WAV)
- Timeouts configurables (`silence_timeout_ms`, `overall_timeout_ms`)
- Logs estructurados con `env_logger` (filtrables por `RUST_LOG`)
- Botón "🗑️ Vaciar caché TTS" en la UI

### FASE 3: El Cuerpo Holográfico ✅

#### FASE 3.1 — Dos ventanas
- Ventana `chat` (500×700, centrada, resizable) — funcionalidad FASE 1+2
- Ventana `avatar` (ajustada al modelo, transparente, sin bordes, sin taskbar)
- Comandos: `toggle_chat_window`, `show_chat_window`, `show_avatar_window`
- `start_dragging`, `close_avatar_window` (cierra app completa)
- `resize_avatar_window` (ajusta ventana al bounding box del modelo)
- CloseRequested interceptado: chat se oculta (no se destruye), avatar cierra app
- Sincronización entre ventanas: evento `stt:state` + `avatar_state_change`
- STT lee modelo de settings (no hardcoded)

#### FASE 3.2 — Avatar 3D con Three.js
- Three.js r170 local (sin CDN, sin import maps)
- GLTFLoader carga `cortana_completa.glb` (Tripo3D, 18k triángulos)
- Materiales originales del modelo (sin shader holográfico)
- Iluminación realista (ambient + key + fill + rim)
- ACESFilmicToneMapping
- Cámara auto-encuadrada desde bounding box (cuerpo completo)
- Modelo arranca de frente (`rotation.y = 0`)

#### FASE 3.3 — Estados reactivos
- **idle**: "En espera" (gris) — respiración + balanceo + head tilt
- **listening**: "Escuchando" (amarillo) — quieta, atenta
- **thinking**: "Pensando" (azul) — erguida, mirando arriba
- **speaking**: "Hablando" (verde) — sway + respiración rápida
- Rotación manual con rueda del mouse
- Indicador de estado con colores y traducción al español

#### FASE 3.4 — Interacción
- Click izquierdo: toggle dictado (solo primer click de ráfaga)
- Triple-click (<600ms): cierra la aplicación
- Click derecho (mousedown button 2): toggle chat
- Rueda: gira modelo
- Drag (>10px): mueve ventana
- `contextmenu` suprimido (preventDefault)
- Auto-send desde el backend (`stt_stop` → `chat.eval` → `sendMessage`)
- Si autoSend activo: no muestra el chat (mensaje ya enviado)
- Si autoSend inactivo: muestra el chat para revisión manual

### Distribución

- Pre-empaquetado de modelos TTS + STT en el bundle (100% offline)
- `setup_bundle_resources()` en `main.rs` detecta resource_dir antes de Tauri
- `setup_gstreamer_plugins()` en `main.rs` configura GST_PLUGIN_PATH antes de WebKitGTK
- GStreamer plugins empaquetados como .tar (evita escaneo de linuxdeploy)
- AppImage + DEB generados con `cargo tauri build`
- Iconos RGBA 128×128 y 512×512

### Bug fixes

- Texto del LLM corrupto ("Cl aro" en vez de "Claro"): removida inserción de espacios entre chunks
- Mensaje enviado 2 veces: removido `sendMessage()` del listener `stt:final`
- Audio reproducido al terminar dictado: removido auto-send del listener `stt:state`
- Click derecho no funcionaba: cambiado de `contextmenu` a `mousedown` button 2
- App no terminaba al cerrar avatar: añadido `app.exit(0)` en `close_avatar_window`
- Chat destruido al cerrar con (X): `CloseRequested` interceptado con `prevent_close` + `hide`
- Segfault al acceder webview desde thread: movido a `stt_stop` (thread principal)
- STT usaba modelo inglés: cambiado default a Whisper medium
- TTS descargaba de internet: `setup_bundle_resources()` en `main.rs` antes de Tauri
- Mensajes truncados: `silence_timeout_ms` 1500→3000, `overall_timeout_ms` 30000→120000
- Doble descarga de voz: `VOICE_LOAD_LOCK` (tokio Mutex) en `ensure_voice_downloaded`
- Texto duplicado/intercalado: `consumedKeys` ya no se borra en `sendMessage`