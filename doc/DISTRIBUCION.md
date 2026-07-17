# SynapseCortana — Estrategia de Distribución e Instalación

## Objetivo

Definir cómo se distribuirá SynapseCortana para que sea:
- **100% local**: sin dependencia de internet después de la instalación
- **Fácil de instalar**: un solo archivo, sin privilegios de administrador
- **Cross-platform**: Linux, Windows y macOS
- **Sin descargas constantes**: los modelos TTS/STT vienen pre-empaquetados

## Estado actual del proyecto

### Lo que ya está embebido en el binario (sin descargas)

| Componente | Ubicación | Tamaño |
|------------|-----------|--------|
| Backend Rust (Tauri) | Compilado en el binario | ~43 MB |
| Frontend HTML/CSS/JS | Embebido vía `frontendDist` | ~1.5 MB |
| Three.js r170 | `frontend/vendor/three.module.js` | ~1.3 MB |
| GLTFLoader | `frontend/vendor/GLTFLoader.js` | ~110 KB |
| Modelo 3D avatar | `frontend/assets/cortana_completa.glb` | ~835 KB |
| sherpa-onnx (lib nativa) | Compilada en el binario | incluida |

### Lo que se descarga de internet en runtime (PROBLEMA)

| Recurso | URL | Tamaño | Momento | Caché |
|---------|-----|--------|---------|-------|
| Voz TTS default | `github.com/k2-fsa/sherpa-onnx/releases/...` | 114 MB | Primera vez | `~/.config/synapse-cortana/voices/` |
| Modelo STT default | `github.com/k2-fsa/sherpa-onnx/releases/...` | 150-350 MB | Primera vez | `~/.config/synapse-cortana/stt-models/` |
| Voces TTS alternativas | GitHub Releases | 22-77 MB c/u | Al seleccionar | sí |
| Modelos STT alternativos | GitHub Releases | 116-350 MB c/u | Al seleccionar | sí |

**Total descargado en un flujo típico (defaults): ~264-464 MB**
**Total worst-case (todos los modelos): ~966 MB**

### Dependencias del sistema operativo

| Dependencia | Linux | Windows | macOS |
|-------------|-------|---------|-------|
| WebKitGTK | `libwebkit2gtk-4.1-dev` | Incluido con Tauri | Incluido con macOS |
| ALSA (audio) | `libasound2-dev` | No necesario (WASAPI) | No necesario (CoreAudio) |
| `tar` + `bzip2` | Preinstalado | **NO incluido** | Preinstalado |
| GTK3 | `libgtk-3-dev` | No necesario | No necesario |

## Estrategia de distribución

### Opción 1 — Pre-empaquetar modelos en el instalador (RECOMENDADA)

**Concepto**: incluir los modelos TTS y STT por defecto dentro del bundle de Tauri como `resources`. En el primer arranque, la app copia los modelos a `~/.config/` desde el bundle en vez de descargarlos.

**Ventajas**:
- 100% offline después de la instalación
- Sin descargas en el primer uso
- Sin dependencia de GitHub
- Experiencia de usuario inmediata

**Desventajas**:
- Instalador más grande (~264-464 MB adicionales)
- Solo se incluyen los modelos por defecto (las voces alternativas seguirían descargándose)

**Implementación**:

#### Paso 1: Descargar modelos pre-build

Crear un script `tools/download_models.sh` que descargue los modelos durante el build y los coloque en `src-tauri/resources/`:

```
src-tauri/resources/
├── voices/
│   └── es_AR-daniela-high/          # 114 MB
│       ├── es_AR-daniela-high.onnx
│       ├── tokens.txt
│       └── espeak-ng-data/
└── stt-models/
    └── sherpa-onnx-whisper-base/    # 150 MB
        ├── base-encoder.onnx
        ├── base-decoder.onnx
        ├── base-tokens.txt
        └── ...
```

#### Paso 2: Configurar `tauri.conf.json` con `resources`

```json
{
  "bundle": {
    "active": true,
    "targets": "all",
    "resources": [
      "resources/voices/es_AR-daniela-high/*",
      "resources/stt-models/sherpa-onnx-whisper-base/*"
    ]
  }
}
```

#### Paso 3: Modificar `tts.rs` y `stt.rs` para buscar modelos del bundle

Antes de descargar de internet, verificar si el modelo ya está en el directorio de recursos del bundle (usando `app.path().resource_dir()`). Si está, copiarlo a `~/.config/` en vez de descargarlo.

```rust
// Pseudocódigo:
fn ensure_voice_downloaded(voice_id: &str, resource_dir: Option<PathBuf>) {
    // 1. ¿Ya está en ~/.config/? → OK, no hacer nada.
    if dir.join("tokens.txt").exists() { return AlreadyPresent; }
    
    // 2. ¿Está en el bundle (resources/)? → copiar.
    if let Some(rd) = resource_dir {
        let bundled = rd.join("voices").join(voice_id);
        if bundled.exists() {
            copy_dir_recursive(bundled, dir);
            return CopiedFromBundle;
        }
    }
    
    // 3. Descargar de internet (fallback).
    download_from_github(voice_id);
}
```

#### Paso 4: Eliminar dependencia de `tar` en Windows

Reemplazar `std::process::Command::new("tar")` con crates Rust puros:
- `bzip2` (descompresión .bz2)
- `tar` (extracción .tar)

Esto elimina la dependencia de `tar` del sistema, critical para Windows donde no existe por defecto.

```toml
# Cargo.toml
bzip2 = "0.4"
tar = "0.4"
```

### Opción 2 — Instalador con descarga diferida (ALTERNATIVA)

**Concepto**: el instalador es pequeño (~43 MB). En el primer arranque, la app descarga los modelos desde GitHub con una barra de progreso.

**Ventajas**:
- Instalador pequeño
- El usuario solo descarga lo que usa

**Desventajas**:
- Requiere internet en el primer uso
- Si GitHub está caído, la app no funciona
- Experiencia de usuario peor (esperar descarga)

### Opción 3 — Mirror propio (HÍBRIDA)

**Concepto**: mantener un mirror de los modelos en un servidor propio (ej. un VPS o GitHub Pages del proyecto). El código intenta primero el mirror, luego GitHub como fallback.

**Ventajas**:
- Disponibilidad garantizada
- Control sobre las versiones

**Desventajas**:
- Requiere mantener un servidor
- Costo de hosting

## Formatos de distribución por plataforma

### Linux — AppImage (recomendado)

```bash
# Build
cargo tauri build

# Output
src-tauri/target/release/bundle/appimage/synapse-cortana_0.1.0_amd64.AppImage
```

| Aspecto | Detalle |
|---------|---------|
| Tamaño sin modelos | ~45 MB |
| Tamaño con modelos | ~310-510 MB |
| Instalación | `chmod +x` y ejecutar |
| Desinstalación | Borrar el archivo |
| Dependencias | Ninguna (todo embebido) |
| Privilegios | No requiere sudo |

### Windows — NSIS installer + portable ZIP

```bash
# Build (cross-compile desde Linux)
cargo tauri build --target x86_64-pc-windows-msvc

# Outputs
bundle/nsis/synapse-cortana_0.1.0_x64-setup.exe    # Instalador
bundle/synapse-cortana_0.1.0_x64_portable.zip      # Portable
```

| Aspecto | Detalle |
|---------|---------|
| Tamaño sin modelos | ~50 MB |
| Tamaño con modelos | ~315-515 MB |
| Instalación | Doble-click → siguiente → siguiente |
| Desinstalación | Add/Remove Programs o borrar carpeta |
| Dependencias | WebView2 (preinstalado en Win10/11) |
| Privilegios | No requiere admin (instalador per-user) |

### macOS — .dmg + .app

```bash
# Build (requiere macOS)
cargo tauri build --target aarch64-apple-darwin  # Apple Silicon
cargo tauri build --target x86_64-apple-darwin   # Intel

# Output
bundle/dmg/synapse-cortana_0.1.0_aarch64.dmg
```

| Aspecto | Detalle |
|---------|---------|
| Tamaño sin modelos | ~55 MB |
| Tamaño con modelos | ~320-520 MB |
| Instalación | Arrastrar .app a Aplicaciones |
| Desinstalación | Arrastrar a Papelera |
| Dependencias | Ninguna (WebKit incluido en macOS) |
| Privilegios | No requiere |

## Dependencias de build (compilación cross-platform)

### Linux (host de build actual)

```bash
sudo apt install libwebkit2gtk-4.1-dev libasound2-dev pkg-config
```

### Windows (cross-compile desde Linux)

```bash
# Instalar toolchain
rustup target add x86_64-pc-windows-msvc

# Requiere Windows SDK o MinGW
# Recomendado: usar GitHub Actions con runner windows-latest
```

### macOS (requiere macOS)

```bash
rustup target add aarch64-apple-darwin
rustup target add x86_64-apple-darwin
# Requiere Xcode Command Line Tools
```

## Pipeline de CI/CD recomendado (GitHub Actions)

```yaml
# .github/workflows/release.yml
name: Release
on:
  push:
    tags: ['v*']

jobs:
  build-linux:
    runs-on: ubuntu-22.04
    steps:
      - uses: actions/checkout@v4
      - run: sudo apt install libwebkit2gtk-4.1-dev libasound2-dev
      - uses: dtolnay/rust-toolchain@stable
      - run: ./tools/download_models.sh  # Pre-descargar modelos
      - run: cargo tauri build
      - uses: actions/upload-artifact@v4
        with:
          name: linux-appimage
          path: src-tauri/target/release/bundle/appimage/*.AppImage

  build-windows:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: ./tools/download_models.ps1
      - run: cargo tauri build
      - uses: actions/upload-artifact@v4
        with:
          name: windows-installer
          path: src-tauri/target/release/bundle/nsis/*.exe

  build-macos:
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: ./tools/download_models.sh
      - run: cargo tauri build --target universal-apple-darwin
      - uses: actions/upload-artifact@v4
        with:
          name: macos-dmg
          path: src-tauri/target/release/bundle/dmg/*.dmg
```

## Endurecimiento de seguridad

### CSP (Content Security Policy)

Reemplazar `"csp": null` con una política restrictiva:

```json
"csp": "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; connect-src 'self' ipc: http://ipc.localhost ws://* http://*; img-src 'self' data: blob:; media-src 'self' data: blob:; worker-src 'self' blob:"
```

Esto permite:
- Scripts solo del bundle local
- WebSocket a cualquier host (gateway configurable)
- Sin conexiones arbitarias del webview

### Verificación de integridad de modelos

Añadir checksums SHA-256 hardcodeados para cada modelo:

```rust
const VOICE_CHECKSUMS: &[(&str, &str)] = &[
    ("es_AR-daniela-high", "a1b2c3d4..."),  // SHA-256 del tarball
];

fn verify_checksum(file: &Path, expected: &str) -> Result<(), String> {
    let actual = sha256_file(file)?;
    if actual != expected {
        return Err(format!("checksum mismatch: expected {expected}, got {actual}"));
    }
    Ok(())
}
```

## Roadmap de implementación

### Fase 1 — Pre-empaquetar modelos (prioridad alta)

1. Crear `tools/download_models.sh` que descargue modelos al directorio `resources/`
2. Añadir `bzip2` y `tar` crates al `Cargo.toml` (eliminar dependencia de `tar` del SO)
3. Modificar `tts.rs` y `stt.rs` para buscar en `resources/` antes de descargar
4. Configurar `tauri.conf.json` con `bundle.resources`
5. Probar que la app funciona 100% offline

### Fase 2 — CI/CD (prioridad media)

1. Crear `.github/workflows/release.yml`
2. Configurar builds para Linux, Windows y macOS
3. Automatizar la creación de tags y releases
4. Subir los artefactos a GitHub Releases

### Fase 3 — Endurecimiento (prioridad baja)

1. Configurar CSP restrictivo
2. Añadir verificación de checksums
3. Cambiar STT a descarga streaming (como TTS)
4. Documentar requisitos mínimos de hardware

## Tamaños estimados del instalador final

| Plataforma | Sin modelos | Con modelo TTS (114 MB) | Con TTS + STT (264 MB) |
|------------|-------------|------------------------|----------------------|
| Linux AppImage | ~45 MB | ~160 MB | ~310 MB |
| Windows NSIS | ~50 MB | ~165 MB | ~315 MB |
| macOS DMG | ~55 MB | ~170 MB | ~320 MB |

## Configuración del usuario (post-instalación)

Todos los settings se guardan en:
- **Linux**: `~/.config/synapse-cortana/`
- **Windows**: `%APPDATA%/synapse-cortana/`
- **macOS**: `~/Library/Application Support/synapse-cortana/`

Contenido:
```
settings.json          # URL del gateway, token, voz, modelo STT, timeouts
device.key             # Identidad Ed25519 del dispositivo
voices/                # Modelos TTS extraídos
stt-models/            # Modelos STT extraídos
tts-cache/             # Caché de audio sintetizado
```

## Conclusión

La estrategia recomendada es **Opción 1 (pre-empaquetar modelos)** con:

1. Modelos TTS + STT incluidos en el instalador (~264 MB adicionales)
2. Crates `bzip2` + `tar` para eliminar dependencia del SO
3. Búsqueda en `resources/` antes de descargar de internet
4. CI/CD con GitHub Actions para builds multiplataforma
5. CSP restrictivo para seguridad

Resultado: un instalador de ~310 MB que funciona 100% offline, sin descargas, sin privilegios de administrador, en Linux, Windows y macOS.

## Autor

SynapseCortana 2026