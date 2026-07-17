# SynapseCortana

> Canal de interfaz visual y auditivo para OpenClaw/Ollama, con avatar 3D holográfico, voz local y dictado por voz — 100% open source, 100% local, sin cloud.

![SynapseCortana](doc/Cortana2.png)

## ¿Qué es SynapseCortana?

SynapseCortana es una aplicación de escritorio que conecta tu ordenador con un gateway de OpenClaw mediante WebSocket, y dota a la IA de **presencia visual** (avatar 3D) y **voz local** (TTS + STT), todo sin depender de servicios cloud.

Inspirado en la naturaleza simbiótica de Cortana en la Silver Timeline de Halo, el proyecto humaniza la interacción hombre-máquina mediante un avatar holográfico dinámico y síntesis de voz local.

### Características principales

- **🔌 Conexión WebSocket** con OpenClaw v4 (handshake Ed25519, token compartido)
- **🗣️ TTS local** con `sherpa-onnx` + voz Piper `es_AR-daniela-high` (femenina, argentina)
- **🎙️ STT local** con Whisper medium (dictado por voz en español)
- **👤 Avatar 3D** con Three.js (modelo importado desde Tripo3D)
- **💬 Chat** con streaming en tiempo real del LLM
- **⚙️ Configuración persistente** (URL del gateway, token, voz, modelo STT, timeouts)
- **🔄 Selector de sesiones** dinámico desde el gateway
- **🎤 Selector de micrófono** para el dictado
- **💾 Caché TTS** en disco (frases repetidas se reproducen instantáneamente)
- **📦 100% offline** después de la instalación (modelos pre-empaquetados)
- **🖥️ Cross-platform** (Linux AppImage, Windows, macOS)

## Arquitectura

```
SynapseCortana/
├── src-tauri/              # Backend Rust (Tauri 2)
│   ├── src/
│   │   ├── main.rs         # Entry point + CLI modes + bundle setup
│   │   ├── lib.rs          # AppState, WebSocket, chat_and_speak, STT, GUI
│   │   ├── tts.rs          # Motor TTS (sherpa-onnx OfflineTts)
│   │   └── stt.rs          # Motor STT (sherpa-onnx Online/OfflineRecognizer)
│   ├── resources/           # Modelos pre-empaquetados (offline)
│   │   ├── voices/         # Voz TTS (114 MB)
│   │   ├── stt-models/      # Modelo STT Whisper medium (900 MB)
│   │   └── gstreamer-plugins.tar  # Plugins de audio (24 MB)
│   ├── tauri.conf.json     # Configuración de ventanas + bundling
│   └── Cargo.toml          # Dependencias Rust
├── frontend/               # Frontend web
│   ├── index.html          # Ventana del chat
│   ├── app.js              # Lógica del chat (~1100 líneas)
│   ├── avatar.html         # Ventana del avatar
│   ├── avatar.js           # Three.js + interacción (~420 líneas)
│   ├── vendor/             # Three.js local (sin CDN)
│   └── assets/             # Modelo 3D (.glb)
├── tools/                  # Herramientas de desarrollo
│   ├── ollama_to_blendermcp.py  # Puente Ollama ↔ Blender
│   ├── export_cortana2.py  # Procesa .glb con Blender
│   └── .venv/               # Entorno Python
└── doc/                    # Documentación de fases
    ├── FASE1.md            # Conexión WebSocket
    ├── FASE2.md            # TTS + STT local
    ├── FASE3.md            # Avatar 3D
    └── DISTRIBUCION.md     # Estrategia de distribución
```

## Instalación

### Linux (AppImage)

```bash
# Descargar el AppImage
chmod +x Synapse\ Cortana_0.1.0_amd64.AppImage
./Synapse\ Cortana_0.1.0_amd64.AppImage
```

Sin necesidad de `sudo`, sin dependencias externas, sin descargas de internet.

### Requisitos del sistema

| Perfil | CPU | RAM | GPU | Disco |
|--------|-----|-----|-----|-------|
| Mínimo | Dual-Core (últimos 6 años) | 4 GB | Integrada | 2 GB |
| Recomendado | Quad-Core+ | 8 GB | Dedicada | 2 GB |

> **Nota**: Si Ollama y OpenClaw corren en la misma máquina, se requieren 8 GB de RAM globales.

## Uso

### Primera configuración

1. Abrir la aplicación → dos ventanas aparecen (avatar + chat)
2. En la pestaña **⚙️ Configuración**:
   - Introducir la URL del gateway OpenClaw (ej. `http://127.0.0.1:18789`)
   - Introducir el token del operador
   - Seleccionar voz TTS (por defecto: `es_AR-daniela-high`, femenina argentina)
   - Seleccionar modelo STT (por defecto: `whisper-medium`, máxima calidad)
   - Seleccionar micrófono
   - Ajustar timeouts si es necesario
3. Pulsar **Conectar**

### Interacción por voz (avatar)

| Acción | Cómo | Resultado |
|--------|------|-----------|
| **Dictar** | Click izquierdo en el avatar | Inicia/detiene dictado por voz |
| **Cerrar** | 3 clicks rápidos (<600ms) | Cierra la aplicación |
| **Chat** | Click derecho | Muestra/oculta la ventana del chat |
| **Girar** | Rueda del mouse | Rota el modelo 3D |
| **Mover** | Click + arrastrar >10px | Mueve la ventana del avatar |

### Interacción por chat

- Escribir mensaje + Enter para enviar
- Click en 🎙️ para dictar desde el chat
- Click en 🔊 para re-reproducir un mensaje
- Checkbox "Enviar automáticamente al terminar de dictar" para envío automático

### Modo CLI (sin GUI)

```bash
# Test de handshake:
./synapse-cortana --cli-test-handshake --url http://127.0.0.1:18789 --token <TOKEN>

# Test de TTS:
./synapse-cortana --cli-test-speak --voice es_AR-daniela-high --text "Hola, soy Cortana" --out /tmp/test.wav

# Logs verbosos:
RUST_LOG=debug ./synapse-cortana
```

## Stack tecnológico

| Componente | Tecnología | Licencia |
|------------|-----------|---------|
| Framework | Tauri 2.x | Apache-2.0/MIT |
| Backend | Rust | MIT/Apache-2.0 |
| Frontend | HTML/CSS/JS (vanilla) | — |
| Avatar 3D | Three.js r170 | MIT |
| TTS | sherpa-onnx + Piper voices | Apache-2.0/MIT |
| STT | sherpa-onnx + Whisper medium | Apache-2.0/MIT |
| WebSocket | tokio-tungstenite | MIT |
| Identidad | Ed25519 (ed25519-dalek) | MIT/Apache-2.0 |
| Audio I/O | cpal (ALSA/CoreAudio/WASAPI) | MIT/Apache-2.0 |

## Roadmap

- ✅ **FASE 1** — Conexión WebSocket con OpenClaw v4
- ✅ **FASE 2** — TTS local + STT + chat integrado + UX + caché
- ✅ **FASE 3** — Avatar 3D + interacción por voz + distribución offline
- 🔄 **FASE 4** — Lip-sync + análisis emocional + expresiones faciales

## Compilación desde código fuente

### Prerequisitos (Linux)

```bash
sudo apt install libwebkit2gtk-4.1-dev libasound2-dev pkg-config librsvg2-dev
```

### Build

```bash
git clone https://github.com/usuario/SynapseCortana.git
cd SynapseCortana/src-tauri
cargo build --release
```

### Generar instaladores

```bash
cargo install tauri-cli --version "^2"
cargo tauri build
# Output: target/release/bundle/appimage/*.AppImage
#         target/release/bundle/deb/*.deb
```

## Documentación

- [FASE 1: El Cascarón Conectivo](doc/FASE1.md) — WebSocket + handshake
- [FASE 2: La Voz de Cortana](doc/FASE2.md) — TTS + STT + chat + UX
- [FASE 3: El Cuerpo Holográfico](doc/FASE3.md) — Avatar 3D + interacción
- [Estrategia de Distribución](doc/DISTRIBUCION.md) — AppImage + DEB + offline

## Licencia

GNU General Public License v3.0. Ver [LICENSE](LICENSE) para más detalles.

## Autor

SynapseCortana 2026