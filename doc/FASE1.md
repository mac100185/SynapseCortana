# Synapse Cortana - FASE 1: El Cascarón Conectivo

## Descripción

FASE 1 del proyecto Synapse-Cortana: establecer la conexión entre una ventana de software externa (Frontend) y el servidor de OpenClaw via WebSocket, según el protocolo oficial v4 de OpenClaw.

## Objetivo

- Lograr que una ventana de software externa reciba y envíe datos a OpenClaw.
- Validar el flujo de datos: enviar un texto y observar las respuestas de Ollama/LLM procesadas por el gateway.

## Estado

✅ **FASE 1 completada y validada end-to-end contra el gateway real.** El binario `synapse-cortana` compila en modo release, el handshake WebSocket se implementa siguiendo el protocolo oficial de OpenClaw v4, y se ha confirmado la conexión exitosa con un gateway OpenClaw v2026.6.6 ejecutándose en una VM remota y accedido a través de un túnel SSH.

### Validación end-to-end

Ejecutado contra `http://127.0.0.1:18789` (gateway reenviado por `ssh -L 18789:127.0.0.1:18789 cortana@192.168.1.59 -N`) con el token del operador:

```
[cli-handshake] Iniciando contra http://127.0.0.1:18789
[cli-handshake] device.id = fc11cc418fcef8c364fe4877f1f0687f313ad0b160ff0f67ac738e19eef443d3
[cli-handshake] publicKey = johT7lFdxA4H7IJPa_82zuepQTBVKnFZRj6iTyRPb5o
[cli-handshake] WS conectado a ws://127.0.0.1:18789
[cli-handshake] challenge recibido, nonce = 134b86ba-e261-4d61-bbab-b068357e2905
[cli-handshake] connect enviado
[cli-handshake] ✅ HANDSHAKE OK
[cli-handshake] protocol = 4
[cli-handshake] server.version = "2026.6.6"
[cli-handshake] connId = "ba0f77b4-f105-4df6-aceb-1c7459856597"
[cli-handshake] auth.role = operator
[cli-handshake] auth.scopes = ["operator.read","operator.write"]
exit=0
```

El gateway además reporta presencia activa para el dispositivo:

```json
{
  "deviceId": "fc11cc418fcef8c364fe4877f1f0687f313ad0b160ff0f67ac738e19eef443d3",
  "host": "gateway-client",
  "mode": "backend",
  "platform": "linux",
  "reason": "connect",
  "roles": ["operator"],
  "scopes": ["operator.read","operator.write"]
}
```

## Estructura del Proyecto

```
SynapseCortana/
├── src-tauri/                  # Backend Rust (Tauri 2)
│   ├── src/
│   │   ├── main.rs             # Entry point: dispatcha --cli-test-handshake o run()
│   │   ├── lib.rs              # Lógica de comandos Tauri + WebSocket + CLI handshake
│   │   └── bin/                # Bins de prueba / andamiaje experimental
│   │       ├── handshake_test.rs
│   │       ├── handshake_test_v2.rs
│   │       ├── persistent_test.rs   # Referencia E2E con clave persistente
│   │       └── sign_test.rs
│   ├── Cargo.toml              # Dependencias Rust
│   ├── build.rs                # Build script
│   └── tauri.conf.json         # Configuración de Tauri
├── frontend/                    # Frontend web
│   ├── index.html              # Interfaz de chat
│   └── app.js                  # Lógica JavaScript (polling de eventos)
└── doc/                        # Documentación
    ├── FASE1.md                # Este archivo
    ├── OpenClaw.txt            # Notas de referencia del protocolo
    └── Synapse_Cortana.docx    # Documento de diseño
```

## Tecnologías

- **Tauri 2.x** — Framework de aplicación ligera.
- **Rust** — Backend con async/await para WebSocket.
- **tokio-tungstenite** — Cliente WebSocket sobre Tokio.
- **WebSocket** — Protocolo de transporte con OpenClaw.
- **OpenClaw** — Gateway de plugins para Ollama.

## Protocolo OpenClaw (referencia rápida)

El protocolo de gateway está documentado en <https://docs.openclaw.ai/es/gateway/protocol>. La FASE 1 implementa:

1. **WebSocket en la raíz** (`{gateway}` sin path `/ws`): el gateway de OpenClaw monta el `WebSocketServer` en la raíz del HTTP server; el path `/ws` causaría `404` / `Connection refused`.
2. **Handshake**:
   - El cliente abre un WebSocket a `ws://{gateway}/`.
   - El gateway envía un evento `connect.challenge` con `{ nonce, ts }`.
   - El cliente responde con una solicitud `connect` que declara `minProtocol`/`maxProtocol`, `client`, `role`, `scopes` y, opcionalmente, `auth.token`.
   - El cliente incluye un bloque `device` con su identidad Ed25519 persistente y la **firma v2 del nonce** (ver "Device identity" más abajo).
   - El gateway responde con `hello-ok` (`payload.type == "hello-ok"`).
3. **RPC posterior**:
   - El cliente envía `{ type: "req", id, method, params }`.
   - El gateway responde con `{ type: "res", id, ok, payload | error }`.
   - Los broadcasts llegan como `{ type: "event", event, payload }`.

### Device identity (Ed25519 v2)

OpenClaw v4 exige que **todas** las conexiones firmen el `connect.challenge` con una clave Ed25519 persistente. La FASE 1 implementa:

- La identidad se genera en el primer arranque con `OsRng` y se guarda como PEM PKCS8 en `~/.config/synapse-cortana/device.key` (permisos `0600`).
- `device.id` = `SHA-256(publicKey_raw_32_bytes)` en hex minúsculas.
- `device.publicKey` = base64url (sin padding) de los 32 bytes raw de la clave pública.
- `device.signature` = base64url de la firma Ed25519 sobre el payload v2:
  ```
  v2|<deviceId>|<clientId>|<clientMode>|<role>|<scopes-csv>|<signedAtMs>|<token>|<nonce>
  ```
  donde `scopes-csv` es la concatenación separada por comas (ej. `operator.read,operator.write`).
- `device.signedAt` = `Date.now()` en milisegundos (timestamp del connect).
- `device.nonce` = el mismo nonce que devolvió el gateway en `connect.challenge` (sin trim, se envía la cadena UUID exacta).

> **Nota sobre v3:** OpenClaw también soporta un payload firmado v3 que añade `platform` y `deviceFamily`. Implementamos **v2** (verificado contra el gateway real con `hello-ok`, role `operator` y scopes preservados) porque la canonicalización exacta de los campos en v3 (orden, casing, separadores) no está documentada y nuestras pruebas contra el gateway v2026.6.6 mostraron que el servidor rechaza la firma v3 con `DEVICE_AUTH_SIGNATURE_INVALID`. El campo `platform` (`linux` / `darwin` / `win32`) sí se envía dentro del bloque `client`, pero no se incluye en el payload firmado. Migrar a v3 queda como mejora futura por si el servidor deja de aceptar v2.

### Identidad de cliente (`client.id` / `client.mode`)

El handshake declara:

- `client.id = "gateway-client"` (uno de los IDs enumerados en `packages/gateway-protocol/src/client-info.ts`).
- `client.mode = "backend"` (el único modo que conserva `scopes` en el flujo de loopback/backend helper sin requerir pairing de dispositivo adicional).
- `client.platform` = `linux` / `darwin` / `win32` (canonicalización de `std::env::consts::OS`).
- `client.version` = `env!("CARGO_PKG_VERSION")`.
- `role = "operator"`, `scopes = ["operator.read", "operator.write"]`.

### Autenticación

- **Token compartido** (recomendado): el operador introduce el token en el campo "Token" del frontend (o se pasa por `--token` en el modo CLI). SynapseCortana lo reenvía como `params.auth.token` y lo concatena (como cadena vacía si no hay) en el payload v2 firmado.
- **Sin token**: el operador puede dejar el campo "Token" vacío. El gateway puede exigir pairing o rechazar la conexión según la política.
- En el caso validado (`gateway-client` + `backend` + `auth.token` + loopback), el handshake se completa en el primer intento y los scopes se preservan sin necesidad de aprobar el dispositivo desde otro cliente.

## Comandos Tauri

| Comando | Tipo | Descripción |
|---------|------|-------------|
| `get_gateway_url` | sync | Devuelve la URL configurada del gateway. |
| `set_gateway_url` | sync | Actualiza la URL del gateway. Valida que sea `http(s)://`. |
| `get_gateway_token` | sync | Devuelve el token configurado. |
| `set_gateway_token` | sync | Actualiza el token. |
| `is_connected` | sync | Indica si hay una conexión WebSocket viva. |
| `check_gateway_connection` | async | Verifica vía HTTP (`/health` o `/`) si el gateway responde. |
| `connect_to_gateway` | async | Inicia el handshake WebSocket. Devuelve `HelloOkInfo` con `protocol`, `server_version`, `conn_id` y `features_methods`. |
| `send_message_to_gateway` | async | Envía un mensaje vía RPC `chat.send` al canal `control-ui`. Devuelve el `id` de la solicitud. |
| `poll_gateway_events` | sync | Drena el buffer de eventos recibidos (chat, agent, etc.) desde el último poll. |
| `disconnect_from_gateway` | async | Cierra la conexión WebSocket. |

## Arquitectura interna

- **`AppState`** es clonable (todos sus campos son `Arc<Mutex/AsyncMutex<...>>`) para que las tareas en background puedan poseer su propia copia.
- El stream WebSocket se divide en dos mitades:
  - **Sink** (escritura) → guardada en `AppState.sink` para enviar RPCs.
  - **Stream** (lectura) → consumida por una tarea en background (`run_event_pump`) que llena el `AppState.inbox` y emite cada evento al frontend vía `app.emit("gateway:event", ...)`.
- El frontend hace `poll_gateway_events` cada 1s para mostrar los mensajes que llegaron del gateway mientras la UI no estaba activa.
- **`DeviceIdentity`** se carga o genera al inicio (vía `DeviceIdentity::load_or_create`). Persiste en disco y contiene:
  - La clave privada Ed25519 (PKCS8 PEM, `0600` en Unix).
  - `device_id()` → SHA-256 de los 32 bytes raw de la pública.
  - `public_key_base64url()` → para el campo `device.publicKey`.
  - `sign_v2(client_id, client_mode, role, scopes, token, nonce, signed_at_ms)` → firma el payload canónico v2 que OpenClaw acepta.
- **WebSocket en raíz**: `build_ws_url` devuelve la URL HTTP tal cual (sin `/ws`), porque el gateway monta el WS en la raíz. Una solicitud a `ws://gateway/ws` devolvería `404`/`Connection refused`.

## Modo CLI de pruebas (sin GUI)

Para entornos sin display (SSH, contenedores, CI) o cuando la GUI de Tauri no se puede inicializar, el binario acepta el flag `--cli-test-handshake` y ejecuta el handshake WebSocket contra el gateway, imprimiendo el resultado en `stderr` y la respuesta JSON pretty en `stdout`. Pensado para validar la conexión sin abrir ventana.

```bash
./target/release/synapse-cortana --cli-test-handshake \
  --url http://127.0.0.1:18789 \
  --token <TOKEN>
```

Flags:
- `--url <URL>`: URL HTTP del gateway (por defecto `http://127.0.0.1:18789`).
- `--token <TOKEN>`: token compartido (alternativa: variable de entorno `OPENCLAW_TOKEN`).

Código de salida: `0` si el handshake termina en `hello-ok`, `1` en cualquier error.

## Compilación

```bash
cd SynapseCortana/src-tauri
cargo build --release
```

## Ejecución

### Modo GUI

```bash
./target/release/synapse-cortana
```

La ventana del frontend se abrirá con la UI de chat. Requiere un display gráfico (X11/Wayland) y las librerías GTK/webkit instaladas (ver "Dependencias del Sistema").

### Modo CLI (sin GUI, ideal para SSH/headless)

```bash
./target/release/synapse-cortana --cli-test-handshake \
  --url http://127.0.0.1:18789 \
  --token <TOKEN>
```

Ver "Modo CLI de pruebas" más arriba para detalles. Útil para depurar el handshake sin abrir ventana.

### Ejemplo: probar contra una VM remota mediante túnel SSH

```bash
# En una terminal: crear el túnel hacia la VM que corre OpenClaw
ssh -L 18789:127.0.0.1:18789 cortana@192.168.1.59 -N

# En otra terminal: ejecutar el handshake contra el puerto reenviado
./target/release/synapse-cortana --cli-test-handshake \
  --url http://127.0.0.1:18789 \
  --token <TOKEN>
```

## Dependencias del Sistema

- GTK3 y librerías de desarrollo asociadas (para Tauri en Linux).
- pkg-config.
- libwebkit2gtk-4.1-dev (en distribuciones basadas en Debian/Ubuntu).

## Próximas Fases

- **FASE 2**: Integración de TTS (Texto a Voz) — usar `talk.speak` o `tts.convert` del gateway.
- **FASE 3**: Avatar 3D holográfico.
- **FASE 4**: Lip-sync y análisis emocional.

## Autor

SynapseCortana 2026
