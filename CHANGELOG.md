# Changelog

Todos los cambios notables de SynapseCortana se documentan en este archivo.

## [0.1.1] — 2026-07-20

### Bug fixes críticos

- **Crash al cerrar app con STT activo**: `close_avatar_window` dropeaba el `JoinHandle` del thread de Whisper sin hacer `join()`, causando que el ONNX Runtime crasheara con `GetElementType is not implemented` al liberar recursos mientras la inferencia seguía en progreso. Ahora se hace `join()` antes de salir, igual que `stt_stop`.
- **Voz TTS y modelo STT se descargaban de internet en vez de copiarse del bundle**: `copy_voice_from_bundle` y `copy_stt_from_bundle` construían la ruta con `bundle_path.join("resources").join(...)`, pero `BUNDLE_RESOURCE_DIR` en modo dev/release ya incluye `resources/`, produciendo una ruta inexistente `resources/resources/`. Ahora se prueban ambas rutas (con y sin `resources/` intermedio). La app ahora cumple la promesa del README: 100% offline después de `download_models.sh`.
- **Sesiones STT solapadas**: cuando el usuario hace click en el avatar para detener el dictado, `stt_stop` bloquea 8-25s en `join()` esperando a que Whisper termine. Durante ese tiempo, el usuario podía iniciar un nuevo dictado (`stt_start`), creando una sesión solapada que perdía la transcripción anterior. Añadido flag `AtomicBool STT_STOPPING` que `stt_start` consulta para rechazar nuevas sesiones mientras el stop del anterior aún no termina.

### Limpieza de código

- **Proyecto 100% limpio**: eliminados todos los warnings de `cargo check` (13 en lib + 6 en bins) y todos los lints de `cargo clippy` (14 adicionales). Cero warnings, cero errores en ambos.
  - Imports muertos removidos (12): `OnlineRecognizer`, `debug`, `error`, `VerifyingKey`, `SystemTime`, `UNIX_EPOCH`, `Signature`, `Signer`, `SigningKey`, `OsRng`, `Digest`, `Sha256`
  - Código muerto removido (4): closure `in_bundle` nunca invocado, loop con body vacío, campo `online` nunca leído, variable `spec` sin uso
  - Construcciones redundantes simplificadas (12): bloque `unsafe` innecesario, casts no-op, `.into()` String→String, `&voice.id` → `voice.id`
  - Visibilidad corregida: `SttInner` ahora `pub` con campos `pub(crate)`

### Limpieza de documentación

- **Comentarios y CHANGELOG sincronizados con el código**: el header de `stt.rs` describía streaming Zipformer como motor principal cuando el default es Whisper medium. El comentario de la sección STT en `lib.rs` prometía un CLI `--cli-test-stt` que no existe. Defaults del doc-comment de `chat_and_speak` corregidos (1500ms→3000ms, 60s→120s). `avatar.js`: eliminada mención de doble-click inexistente, corregido `<500ms`→`<600ms`.
- **`tauri.conf.json`**: removido `$schema` que apuntaba a URL 404 (el sitio de Tauri se reorganizó).
- **`mock_gateway.py`**: fix de type checker — `from websockets import serve` con `# type: ignore` (la librería 10.4 de apt no incluye type stubs).

### Verificación E2E

Probado por el usuario en AppImage real contra gateway OpenClaw:
- ✅ AppImage arranca 100% offline (voz + STT + GStreamer copiados del bundle)
- ✅ Conexión al gateway (protocol v4, hello-ok)
- ✅ Chat texto → LLM → TTS (4 ciclos completos)
- ✅ STT dictado → transcripción → auto-send (3 transcripciones correctas en español)
- ✅ Triple-click con STT activo: cierra limpio (sin crash)
- ✅ Sesiones STT no solapadas
- ✅ Conversación idéntica entre dashboard OpenClaw y SynapseCortana

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