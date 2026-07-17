# Contribuir a SynapseCortana

¡Gracias por tu interés en contribuir! Este documento explica cómo configurar el entorno de desarrollo y enviar cambios.

## Requisitos previos

### Linux (Ubuntu/Debian)

```bash
sudo apt install libwebkit2gtk-4.1-dev libasound2-dev pkg-config librsvg2-dev
```

### Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup default stable
```

### Tauri CLI

```bash
cargo install tauri-cli --version "^2"
```

## Setup del proyecto

```bash
git clone https://github.com/usuario/SynapseCortana.git
cd SynapseCortana

# Descargar modelos pre-empaquetados (necesarios para build offline)
./tools/download_models.sh

# Compilar
cd src-tauri
cargo build --release

# Ejecutar
./target/release/synapse-cortana

# Generar instaladores
cargo tauri build
```

## Estructura del proyecto

```
frontend/          # Frontend web (HTML/CSS/JS, sin framework)
src-tauri/         # Backend Rust (Tauri 2)
  src/main.rs     # Entry point, CLI modes, bundle setup
  src/lib.rs      # AppState, WebSocket, chat, STT, GUI commands
  src/tts.rs      # Motor TTS (sherpa-onnx)
  src/stt.rs      # Motor STT (sherpa-onnx Whisper)
  resources/       # Modelos pre-empaquetados (NO subir a git)
tools/            # Scripts de desarrollo
doc/              # Documentación de fases
```

## Convenciones de código

### Rust
- Usar `log::{info, warn, error, debug}` (no `eprintln!` ni `println!`)
- Logs filtrables con `RUST_LOG=debug ./synapse-cortana`
- Comentarios en español
- `#[tauri::command]` para cada función expuesta al frontend

### JavaScript
- Sin framework (vanilla JS)
- `invoke("command_name", { args })` para IPC con Rust
- `listen("event_name", callback)` para eventos del backend
- Comentarios en español

### Documentación
- Cada fase tiene su propio `doc/FASEn.md`
- `CHANGELOG.md` para todos los cambios
- `README.md` para usuarios finales

## Flujo de trabajo

1. Crear un branch: `git checkout -b feature/nueva-funcionalidad`
2. Hacer cambios
3. Compilar y testear: `cargo build --release && ./target/release/synapse-cortana`
4. Commit: `git commit -m "feat: descripción del cambio"`
5. Push: `git push origin feature/nueva-funcionalidad`
6. Crear Pull Request

## Tests

### Test 1: Handshake (sin GUI)
```bash
./target/release/synapse-cortana --cli-test-handshake \
    --url http://127.0.0.1:18789 --token <TOKEN>
```

### Test 2: TTS (sin GUI)
```bash
./target/release/synapse-cortana --cli-test-speak \
    --voice es_AR-daniela-high --text "Hola" --out /tmp/test.wav
```

### Test 3: GUI completa
```bash
RUST_LOG=info ./target/release/synapse-cortana
```

## Licencia

GNU GPLv3. Al contribuir, aceptas que tus cambios se publiquen bajo la misma licencia.