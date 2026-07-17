# SynapseCortana

> Canal de interfaz visual y auditivo para OpenClaw/Ollama, con avatar 3D holográfico, voz local y dictado por voz — 100% open source, 100% local, sin cloud.

![SynapseCortana](doc/Cortana2.png)

## ¿Qué es SynapseCortana?

SynapseCortana es una aplicación de escritorio que conecta tu ordenador con un gateway de OpenClaw mediante WebSocket, y dota a la IA de **presencia visual** (avatar 3D) y **voz local** (TTS + STT), todo sin depender de servicios cloud.

Inspirado en la naturaleza simbiótica de Cortana en la Silver Timeline de Halo, el proyecto humaniza la interacción hombre-máquina mediante un avatar holográfico dinámico y síntesis de voz local.

### Características principales

- **🔌 Conexión WebSocket** con OpenClaw v4 (handshake Ed25519, token compartido)
- **🗣️ TTS local** con `sherpa-onnx` + voz Piper `es_AR-daniela-high` (femenina, argentina)
- **🎙️ STT local** con Whisper medium (dictado por voz en español, máxima calidad)
- **👤 Avatar 3D** con Three.js (modelo importado desde Tripo3D)
- **💬 Chat** con streaming en tiempo real del LLM
- **⚙️ Configuración persistente** (URL del gateway, token, voz, modelo STT, timeouts)
- **🔄 Selector de sesiones** dinámico desde el gateway
- **🎤 Selector de micrófono** para el dictado
- **💾 Caché TTS** en disco (frases repetidas se reproducen instantáneamente)
- **📦 100% offline** después de la instalación (modelos pre-empaquetados)
- **🖥️ Cross-platform** (Linux AppImage, Windows, macOS)

---

## Guía rápida para usuarios

### Opción A — Descargar el AppImage (recomendado, sin compilar)

1. Ir a la página de [Releases](https://github.com/usuario/SynapseCortana/releases) del repositorio
2. Descargar `Synapse Cortana_0.1.0_amd64.AppImage` (771 MB, incluye todo: voz TTS + modelo STT + plugins de audio)
3. **Dar permisos de ejecución**:
   ```bash
   chmod +x "Synapse Cortana_0.1.0_amd64.AppImage"
   ```
4. **Ejecutar**:
   ```bash
   .//"Synapse Cortana_0.1.0_amd64.AppImage"
   ```

Sin necesidad de `sudo`, sin dependencias externas, sin descargas de internet. El AppImage incluye todos los modelos pre-empaquetados.

> **Nota**: Si no encuentras el AppImage en Releases, puedes compilarlo desde código fuente (ver sección [Guía para desarrolladores](#guía-para-desarrolladores) más abajo).

### Opción B — Conectar a un gateway OpenClaw remoto mediante túnel SSH

Si tu gateway de OpenClaw está en otra máquina (ej. un servidor o VM), puedes acceder mediante un túnel SSH:

```bash
# Terminal 1: crear el túnel SSH al gateway remoto
ssh -L 18789:127.0.0.1:18789 cortana@192.168.1.59 -N

# Terminal 2: ejecutar SynapseCortana
./"Synapse Cortana_0.1.0_amd64.AppImage"
```

Luego en la aplicación:
1. Se abren dos ventanas: el **avatar** (transparente) y el **chat**
2. En el chat, ve a la pestaña **⚙️ Configuración**
3. Introduce:
   - **URL del gateway**: `http://127.0.0.1:18789`
   - **Token**: el token del operador de OpenClaw (lo encuentras en `~/.openclaw/openclaw.json` en el servidor)
4. Pulsar **Conectar** — debe aparecer "✅ Conectado al Gateway via WebSocket protocolo v4"
5. ¡Listo! Escribe un mensaje o haz click en el avatar para dictar por voz

### Requisitos del sistema

| Perfil | CPU | RAM | GPU | Disco |
|--------|-----|-----|-----|-------|
| Mínimo | Dual-Core (últimos 6 años) | 4 GB | Integrada | 2 GB |
| Recomendado | Quad-Core+ | 8 GB | Dedicada | 2 GB |

> **Nota**: Si Ollama y OpenClaw corren en la misma máquina, se requieren 8 GB de RAM globales.

### Interacción por voz (avatar)

| Acción | Cómo | Resultado |
|--------|------|-----------|
| **Dictar** | Click izquierdo en el avatar | Inicia/detiene dictado por voz |
| **Cerrar** | 3 clicks rápidos (<600ms) | Cierra la aplicación |
| **Chat** | Click derecho | Muestra/oculta la ventana del chat |
| **Girar** | Rueda del mouse | Rota el modelo 3D |
| **Mover** | Click + arrastrar >10px | Mueve la ventana del avatar |

### Interacción por chat

- Escribir mensaje + **Enter** para enviar
- Click en **🎙️** para dictar desde el chat
- Click en **🔊** junto a un mensaje para re-reproducirlo
- Checkbox **"Enviar automáticamente al terminar de dictar"** para envío automático
- Selector de **sesión** del gateway (dropdown)
- Selector de **voz TTS** y **modelo STT**
- Selector de **micrófono**
- Ajuste de **timeouts** (silencio y global)

### Estados del avatar

El avatar muestra un indicador en la parte inferior con colores:

| Estado | Texto | Color | Significado |
|--------|-------|-------|------------|
| idle | En espera | Gris | Sin actividad |
| listening | Escuchando | Amarillo | Dictando por voz |
| thinking | Pensando | Azul | LLM procesando respuesta |
| speaking | Hablando | Verde | TTS reproduciendo audio |

### Modo CLI (sin GUI, para testing)

```bash
# Test de handshake con el gateway:
./synapse-cortana --cli-test-handshake \
    --url http://127.0.0.1:18789 \
    --token <TOKEN>

# Test de TTS (sintetiza voz y guarda WAV):
./synapse-cortana --cli-test-speak \
    --voice es_AR-daniela-high \
    --text "Hola, soy Cortana" \
    --out /tmp/test.wav

# Logs verbosos para depuración:
RUST_LOG=debug ./synapse-cortana
RUST_LOG=synapse_cortana=trace ./synapse-cortana
```

---

## Guía para desarrolladores

### Compilación desde código fuente

#### Prerequisitos (Linux Ubuntu/Debian)

```bash
sudo apt install libwebkit2gtk-4.1-dev libasound2-dev pkg-config librsvg2-dev
```

#### Build

```bash
git clone https://github.com/usuario/SynapseCortana.git
cd SynapseCortana

# IMPORTANTE: descargar modelos TTS + STT + GStreamer antes de compilar
# Esto descarga ~1 GB de modelos necesarios para que la app funcione 100% offline
./tools/download_models.sh

cd src-tauri
cargo build --release
```

#### Generar instaladores (Linux)

```bash
cargo install tauri-cli --version "^2"
cargo tauri build
# Output: target/release/bundle/appimage/*.AppImage
#         target/release/bundle/deb/*.deb
```

> **Nota**: `download_models.sh` solo necesita ejecutarse una vez. Los modelos se guardan en `src-tauri/resources/` y no se suben a git (están en `.gitignore`). Sin estos modelos, la app descargará de internet en el primer uso (~1 GB).

### Compilación para Windows

#### Prerequisitos (Windows)

1. **Instalar [Visual Studio C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)** — seleccionar "Desktop development with C++"
2. **Instalar [Rust](https://rustup.rs/)** — descargar y ejecutar `rustup-init.exe`
3. **Instalar [Node.js](https://nodejs.org/)** 18+ (necesario para algunas herramientas de Tauri)
4. **Instalar [Git](https://git-scm.com/)**

#### Build (Windows)

```powershell
git clone https://github.com/mac100185/SynapseCortana.git
cd SynapseCortana

# Descargar modelos (requiere bash — usar Git Bash o WSL):
bash tools/download_models.sh

# Compilar:
cd src-tauri
cargo build --release
```

#### Generar instalador (Windows)

```powershell
cargo install tauri-cli --version "^2"
cargo tauri build --bundles nsis
# Output: target/release/bundle/nsis/Synapse Cortana_0.1.0_x64-setup.exe
```

El instalador NSIS no requiere privilegios de administrador. Se instala como aplicación de usuario.

> **Importante para Windows**:
> - El script `download_models.sh` requiere **Git Bash** o **WSL** para ejecutarse en Windows.
> - Alternativa: descargar manualmente los modelos desde los enlaces en `tools/download_models.sh` y colocarlos en `src-tauri/resources/voices/es_AR-daniela-high/` y `src-tauri/resources/stt-models/sherpa-onnx-whisper-medium/`.
> - Los plugins de GStreamer no son necesarios en Windows (Tauri usa WebView2 que incluye su propio motor de audio).
> - Windows 10/11 incluye WebView2 por defecto. Si no, descargarlo desde [aka.ms/webview2](https://go.microsoft.com/fwlink/p/?LinkId=2124705).

### Compilación para macOS

#### Prerequisitos (macOS)

1. **Instalar Xcode Command Line Tools**:
   ```bash
   xcode-select --install
   ```
2. **Instalar [Rust](https://rustup.rs/)**:
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```
3. **Instalar [Homebrew](https://brew.sh/)** (si no está instalado)

#### Build (macOS)

```bash
git clone https://github.com/mac100185/SynapseCortana.git
cd SynapseCortana

# Descargar modelos
./tools/download_models.sh

# Compilar
cd src-tauri
cargo build --release
```

#### Generar instalador (macOS)

```bash
cargo install tauri-cli --version "^2"

# Para Apple Silicon (M1/M2/M3):
cargo tauri build --target aarch64-apple-darwin --bundles dmg
# Output: target/aarch64-apple-darwin/release/bundle/dmg/*.dmg

# Para Intel:
cargo tauri build --target x86_64-apple-darwin --bundles dmg
# Output: target/x86_64-apple-darwin/release/bundle/dmg/*.dmg
```

El archivo `.dmg` se arrastra a la carpeta Aplicaciones para instalar. No requiere permisos especiales.

> **Nota para macOS**:
> - Los plugins de GStreamer no son necesarios en macOS (Tauri usa WebKit nativo de macOS que incluye su propio motor de audio).
> - El script `download_models.sh` copiará los plugins de GStreamer del sistema si existen, pero en macOS no se usan.
> - Para crear un binario universal (Apple Silicon + Intel): `cargo tauri build --target universal-apple-darwin --bundles dmg`

### Resumen de plataformas

| Plataforma | Prerequisitos | Instalador | Tamaño aprox. |
|------------|--------------|-----------|---------------|
| Linux | `libwebkit2gtk-4.1-dev`, `libasound2-dev`, `pkg-config`, `librsvg2-dev` | AppImage + DEB | ~771 MB |
| Windows | Visual Studio C++ Build Tools, WebView2 | NSIS .exe | ~500 MB |
| macOS | Xcode Command Line Tools | .dmg | ~550 MB |

### Tests

```bash
# Test 1: Handshake (sin GUI, sin TTS)
./target/release/synapse-cortana --cli-test-handshake \
    --url http://127.0.0.1:18789 --token <TOKEN>

# Test 2: TTS local (sin GUI, sin gateway)
./target/release/synapse-cortana --cli-test-speak \
    --voice es_AR-daniela-high --text "Hola" --out /tmp/test.wav

# Test 3: GUI completa (con display)
RUST_LOG=info ./target/release/synapse-cortana
```

---

## Herramientas de desarrollo (`tools/`)

La carpeta `tools/` contiene scripts utilitarios para desarrollo. **No son necesarios para usar la aplicación**, solo para modificar modelos 3D o automatizar tareas.

### `download_models.sh` — Descargar modelos pre-empaquetados

Descarga los modelos TTS, STT y plugins de GStreamer necesarios para que la app funcione 100% offline.

```bash
./tools/download_models.sh
```

Descarga:
- Voz TTS `es_AR-daniela-high` (114 MB) desde GitHub Releases de k2-fsa
- Modelo STT Whisper medium int8 (900 MB) desde GitHub Releases de k2-fsa
- Plugins de GStreamer (24 MB) desde el sistema local

Los modelos se guardan en `src-tauri/resources/` y se incluyen en el AppImage/DEB.

### `ollama_to_blendermcp.py` — Puente Ollama ↔ BlenderMCP

Conecta Ollama (con `minimax-m3:cloud`) a Blender mediante el addon BlenderMCP. Permite generar modelos 3D desde texto e imagen usando IA.

```bash
# Setup (una vez):
pip install ollama mcp  # o usar tools/.venv

# Modo agente (generar modelo desde prompt + imagen):
tools/.venv/bin/python tools/ollama_to_blendermcp.py "Crea una cabeza humanoide..."

# Modo MCP server (para Claude Desktop, Cursor, etc.):
tools/.venv/bin/python tools/ollama_to_blendermcp.py --mcp

# Inspeccionar escena de Blender:
tools/.venv/bin/python -c "
import sys; sys.path.insert(0, 'tools')
from ollama_to_blendermcp import BlenderMCPClient
print(BlenderMCPClient().get_scene_info())
"
```

Requisitos:
- Blender 4.x corriendo con el addon BlenderMCP activo (socket en `localhost:9876`)
- Ollama corriendo con modelo `minimax-m3:cloud` (visión + tool-calling)

### `analyze_glb.py` — Analizar un archivo .glb

Analiza un modelo .glb desde Blender: vértices, triángulos, materiales, bounding box.

```bash
tools/.venv/bin/python -c "
import sys; sys.path.insert(0, 'tools')
from ollama_to_blendermcp import BlenderMCPClient
c = BlenderMCPClient()
code = open('tools/analyze_glb.py').read()
print(c.execute_code(code))
"
```

### `export_cortana2.py` — Procesar modelo 3D con Blender

Decima, re-escala y exporta un modelo .glb conservando los materiales originales.

```bash
tools/.venv/bin/python -c "
import sys; sys.path.insert(0, 'tools')
from ollama_to_blendermcp import BlenderMCPClient
c = BlenderMCPClient()
code = open('tools/export_cortana2.py').read()
print(c.execute_code(code))
"
```

### `setup_sculpt_scene.py` — Preparar escena para esculpir

Configura Blender con imagen de referencia, material holográfico y esferas base para esculpir manualmente.

```bash
tools/.venv/bin/python -c "
import sys; sys.path.insert(0, 'tools')
from ollama_to_blendermcp import BlenderMCPClient
c = BlenderMCPClient()
code = open('tools/setup_sculpt_scene.py').read()
print(c.execute_code(code))
"
```

### `export_sculpted_model.py` — Exportar modelo esculpido

Une todas las mallas, aplica modificadores y exporta a `.glb`.

### `prepare_user_glb.py` — Decima + material holográfico

Versión anterior del procesamiento con shader holográfico azul. Usar `export_cortana2.py` para materiales originales.

### `export_original_materials.py` — Exportar con materiales originales

Alternativa a `export_cortana2.py`. Decima y re-escala sin cambiar materiales.

### `blender-mcp/` — Addon BlenderMCP

Código del addon BlenderMCP (de [ahujasid/blender-mcp](https://github.com/ahujasid/blender-mcp)) ya instalado en `~/.config/blender/4.0/scripts/addons/blender_mcp/`.

---

## Arquitectura

```
SynapseCortana/
├── src-tauri/              # Backend Rust (Tauri 2)
│   ├── src/
│   │   ├── main.rs         # Entry point, CLI modes, bundle setup, GStreamer
│   │   ├── lib.rs          # AppState, WebSocket, chat_and_speak, STT, GUI
│   │   ├── tts.rs          # Motor TTS (sherpa-onnx OfflineTts + Piper voices)
│   │   └── stt.rs          # Motor STT (sherpa-onnx Whisper medium)
│   ├── resources/           # Modelos pre-empaquetados (NO en git, ver .gitignore)
│   │   ├── voices/         # Voz TTS es_AR-daniela-high (114 MB)
│   │   ├── stt-models/      # Modelo STT Whisper medium int8 (900 MB)
│   │   └── gstreamer-plugins.tar  # Plugins de audio GStreamer (24 MB)
│   ├── tauri.conf.json     # Configuración de ventanas + bundling
│   └── Cargo.toml          # Dependencias Rust
├── frontend/               # Frontend web (vanilla JS, sin framework)
│   ├── index.html          # Ventana del chat (pestañas Config/Chat)
│   ├── app.js              # Lógica del chat (~1100 líneas)
│   ├── avatar.html         # Ventana del avatar (canvas + Three.js)
│   ├── avatar.js           # Three.js + interacción (~420 líneas)
│   ├── vendor/             # Three.js r170 local (sin CDN)
│   │   ├── three.module.js
│   │   └── GLTFLoader.js
│   └── assets/             # Modelo 3D (.glb)
│       ├── Cortana.glb      # Modelo original de Tripo3D (6 MB)
│       ├── Cortana2.glb     # Modelo v2 de Tripo3D (6 MB)
│       └── cortana_completa.glb  # Modelo procesado (835 KB, 18k tris)
├── tools/                  # Herramientas de desarrollo (ver sección arriba)
│   ├── download_models.sh  # Descargar modelos para build offline
│   ├── ollama_to_blendermcp.py  # Puente Ollama ↔ Blender
│   ├── blender-mcp/        # Addon BlenderMCP
│   ├── analyze_glb.py      # Analizar .glb
│   ├── export_cortana2.py   # Procesar .glb (decima + re-escala)
│   ├── setup_sculpt_scene.py  # Setup para esculpir
│   ├── export_sculpted_model.py  # Exportar modelo esculpido
│   ├── prepare_user_glb.py  # Decima + material holográfico
│   └── export_original_materials.py  # Exportar con materiales originales
└── doc/                    # Documentación de fases
    ├── FASE1.md            # Conexión WebSocket
    ├── FASE2.md            # TTS + STT local
    ├── FASE3.md            # Avatar 3D
    ├── DISTRIBUCION.md     # Estrategia de distribución
    └── SynapseCortana.md  # Documento original del proyecto
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
| Modelos 3D | Tripo3D + Blender | — |
| CI/CD | GitHub Actions | — |

## Roadmap

- ✅ **FASE 1** — Conexión WebSocket con OpenClaw v4
- ✅ **FASE 2** — TTS local + STT + chat integrado + UX + caché
- ✅ **FASE 3** — Avatar 3D + interacción por voz + distribución offline
- 🔄 **FASE 4** — Lip-sync + análisis emocional + expresiones faciales

## Documentación

- [📖 Manual de Usuario](doc/MANUAL.md) — **Guía completa de la interfaz gráfica**: todas las opciones del chat, controles del avatar, flujo por voz, flujo por chat, solución de problemas
- [FASE 1: El Cascarón Conectivo](doc/FASE1.md) — WebSocket + handshake
- [FASE 2: La Voz de Cortana](doc/FASE2.md) — TTS + STT + chat + UX
- [FASE 3: El Cuerpo Holográfico](doc/FASE3.md) — Avatar 3D + interacción
- [Estrategia de Distribución](doc/DISTRIBUCION.md) — AppImage + DEB + offline
- [Cómo contribuir](CONTRIBUTING.md) — Setup, convenciones, tests
- [Historial de cambios](CHANGELOG.md) — Todas las versiones

## Licencia

GNU General Public License v3.0. Ver [LICENSE](LICENSE) para más detalles.

## Autor

Alan Mac-Arthur García Díaz — 2026