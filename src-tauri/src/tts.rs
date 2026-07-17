//! TTS local open source (FASE 2).
//!
//! Wrapper alrededor de `sherpa-onnx` que carga modelos VITS/Piper
//! pre-entrenados en español desde HuggingFace. Diseñado para evitar
//! cualquier dependencia de TTS cloud (Edge, Azure, ElevenLabs, etc.).
//!
//! ## Voces soportadas
//!
//! Las voces se declaran en [`VoiceSpec`]. Cada voz tiene un `id` (estable,
//! para usar en configuración) y dos archivos a descargar de
//! `huggingface.co/rhasspy/piper-voices`:
//!   - `<voice_id>.onnx`       — el modelo VITS.
//!   - `<voice_id>.onnx.json`  — metadatos (sample rate, tokens, etc.).
//!
//! ## Persistencia
//!
//! Los modelos se cachean en `~/.config/synapse-cortana/voices/<voice_id>/`
//! (gestionado por `directories`). La primera vez que se invoca una voz, se
//! descarga; las siguientes se reutiliza el archivo local.
//!
//! ## Licencia
//!
//! `sherpa-onnx` es Apache-2.0. Los modelos `rhasspy/piper-voices` son MIT.
//! No usamos `piper-rs` (que enlaza libpiper GPL-3.0).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex as AsyncMutex;

use sherpa_onnx::{OfflineTts, OfflineTtsConfig, OfflineTtsModelConfig, OfflineTtsVitsModelConfig};

// ============================================
// ESPECIFICACIONES DE VOCES
// ============================================

/// Descriptor de una voz Piper pre-entrenada.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VoiceSpec {
    /// ID estable. Se usa como nombre de carpeta en disco y para
    /// `tts_set_voice`. Ej.: `es_ES-davefx-medium`.
    pub id: &'static str,
    /// Etiqueta legible para mostrar en la UI. Ej.: "Castellano (varón, ES)".
    pub label: &'static str,
    /// Locale BCP-47. Ej.: `es-ES`, `es-MX`, `es-AR`.
    pub locale: &'static str,
    /// URL del **tarball oficial** pre-empaquetado por `k2-fsa` en
    /// `github.com/k2-fsa/sherpa-onnx/releases/tag/tts-models`. Cada
    /// tarball contiene el `.onnx` con metadatos embebidos, el
    /// `tokens.txt` y el `espeak-ng-data/`.
    /// Ej.: `https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/vits-piper-es_ES-davefx-medium.tar.bz2`.
    pub tarball_url: &'static str,
    /// Tamaño aproximado en MB (informativo, para mostrar en UI).
    pub size_mb_approx: u32,
}

/// Catálogo de voces disponibles en esta build.
///
/// Los tarballs son los oficiales de `k2-fsa/sherpa-onnx` (Apache-2.0),
/// que reempaquetan los modelos Piper con los metadatos y tokens
/// que `sherpa-onnx` necesita.
pub const VOICE_CATALOG: &[VoiceSpec] = &[
    VoiceSpec {
        id: "es_ES-davefx-medium",
        label: "Castellano (varón, ES) — davefx medium",
        locale: "es-ES",
        tarball_url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/vits-piper-es_ES-davefx-medium.tar.bz2",
        size_mb_approx: 64,
    },
    VoiceSpec {
        id: "es_ES-sharvard-medium",
        label: "Castellano (varón, ES) — sharvard medium",
        locale: "es-ES",
        tarball_url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/vits-piper-es_ES-sharvard-medium.tar.bz2",
        size_mb_approx: 77,
    },
    VoiceSpec {
        id: "es_MX-ald-medium",
        label: "Mexicano (varón, MX) — ald medium",
        locale: "es-MX",
        tarball_url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/vits-piper-es_MX-ald-medium.tar.bz2",
        size_mb_approx: 63,
    },
    VoiceSpec {
        id: "es_AR-daniela-high",
        label: "Argentina (mujer, AR) — daniela high",
        locale: "es-AR",
        tarball_url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/vits-piper-es_AR-daniela-high.tar.bz2",
        size_mb_approx: 114,
    },
    // Voz femenina alternativa, calidad baja (~20-25 MB), más rápida
    // que `daniela-high` pero suena más robótica. Útil como fallback
    // cuando la latencia es prioritaria.
    VoiceSpec {
        id: "es_ES-mls_9972-low",
        label: "Castellana (mujer, ES) — mls_9972 low",
        locale: "es-ES",
        tarball_url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/vits-piper-es_ES-mls_9972-low.tar.bz2",
        size_mb_approx: 22,
    },
];

/// Voz por defecto. Voz femenina argentina de alta calidad, 114 MB.
/// Es la mejor voz femenina open source en español disponible en
/// `rhasspy/piper-voices` (no hay voces femeninas `es_ES` de calidad
/// media o alta). Descarga ~1 min la primera vez.
pub const DEFAULT_VOICE_ID: &str = "es_AR-daniela-high";

/// Busca una voz por su `id`. Devuelve `None` si no existe.
pub fn voice_by_id(id: &str) -> Option<&'static VoiceSpec> {
    VOICE_CATALOG.iter().find(|v| v.id == id)
}

// ============================================
// RUTAS EN DISCO
// ============================================

/// Directorio base donde se cachean los modelos de voz.
/// `~/.config/synapse-cortana/voices/` (en Linux).
pub fn voices_root() -> Option<PathBuf> {
    let base = directories::ProjectDirs::from("ai", "openclaw", "synapse-cortana")?;
    Some(base.config_dir().join("voices"))
}

/// Directorio específico para una voz. Tras extraer el tarball oficial
/// de `k2-fsa`, la estructura es:
///   `<voice_dir>/<voice_id>.onnx`
///   `<voice_dir>/<voice_id>.onnx.json`  (opcional, referencia)
///   `<voice_dir>/tokens.txt`
///   `<voice_dir>/espeak-ng-data/`
pub fn voice_dir(voice_id: &str) -> Option<PathBuf> {
    Some(voices_root()?.join(voice_id))
}

/// Ruta al archivo `.onnx` de una voz.
pub fn voice_onnx_path(voice_id: &str) -> Option<PathBuf> {
    Some(voice_dir(voice_id)?.join(format!("{voice_id}.onnx")))
}

/// Ruta al `tokens.txt` (formato sherpa-onnx, **NO** el `.onnx.json`).
pub fn voice_tokens_path(voice_id: &str) -> Option<PathBuf> {
    Some(voice_dir(voice_id)?.join("tokens.txt"))
}

/// Ruta al directorio `espeak-ng-data/` (diccionarios fonéticos).
pub fn voice_espeak_data_path(voice_id: &str) -> Option<PathBuf> {
    Some(voice_dir(voice_id)?.join("espeak-ng-data"))
}

// ============================================
// DESCARGA DE MODELOS
// ============================================

/// Resultado de la descarga: `Downloaded` si se descargó de cero,
/// `AlreadyPresent` si ya estaba en disco.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadOutcome {
    AlreadyPresent,
    Downloaded,
}

/// Asegura que el modelo de voz está extraído en disco. Si no está,
/// descarga el tarball oficial de `k2-fsa` y lo extrae.
///
/// Estructura esperada tras la extracción:
///   `<voice_dir>/<voice_id>.onnx`
///   `<voice_dir>/tokens.txt`
///   `<voice_dir>/espeak-ng-data/`
/// Directorio donde Tauri empaqueta los recursos del bundle.
/// En runtime, esto apunta al directorio `resources/` dentro del
/// AppImage/DEB instalado. Si la app se ejecuta desde el binario
/// suelto (sin instalar), retorna None y se usa el fallback de internet.
static BUNDLE_RESOURCE_DIR: std::sync::OnceLock<Option<PathBuf>> = std::sync::OnceLock::new();

/// Mutex async para prevenir que dos threads descarguen/copien la misma voz
/// simultáneamente (pre-carga + frontend). Sin esto, las dos
/// operaciones compiten y corrompen el .onnx.
static VOICE_LOAD_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Inicializa el directorio de recursos del bundle. Llamado desde
/// `setup` en `tauri::Builder`.
pub fn init_bundle_resources(path: PathBuf) {
    let _ = BUNDLE_RESOURCE_DIR.set(Some(path));
}

/// Busca una voz en el directorio de recursos del bundle. Si la
/// encuentra, la copia a `~/.config/synapse-cortana/voices/<id>/`.
/// Retorna true si copió con éxito, false si no hay bundle o no
/// encontró la voz.
fn copy_voice_from_bundle(voice_id: &str, target_dir: &Path) -> bool {
    let Some(bundle_dir) = BUNDLE_RESOURCE_DIR.get() else {
        return false;
    };
    let Some(bundle_path) = bundle_dir else {
        return false;
    };
    let src = bundle_path.join("resources").join("voices").join(voice_id);
    if !src.exists() {
        return false;
    }
    info!(
        "[tts] copiando voz '{}' desde el bundle (offline)",
        voice_id
    );
    copy_dir_recursive(&src, target_dir)
}

/// Copia un directorio recursivamente.
fn copy_dir_recursive(src: &Path, dst: &Path) -> bool {
    if let Err(e) = std::fs::create_dir_all(dst) {
        warn!("[tts] crear dir {}: {e}", dst.display());
        return false;
    }
    let entries = match std::fs::read_dir(src) {
        Ok(e) => e,
        Err(e) => {
            warn!("[tts] read_dir {}: {e}", src.display());
            return false;
        }
    };
    for entry in entries.flatten() {
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            if !copy_dir_recursive(&from, &to) {
                return false;
            }
        } else {
            if let Err(e) = std::fs::copy(&from, &to) {
                warn!("[tts] copiar {} → {}: {e}", from.display(), to.display());
                return false;
            }
        }
    }
    true
}

pub async fn ensure_voice_downloaded(voice: &VoiceSpec) -> Result<DownloadOutcome, String> {
    // Adquirir el lock ANTES de verificar si la voz ya existe.
    // Esto previene que la pre-carga (background) y el frontend
    // (tts_set_voice) ejecuten ensure_voice_downloaded simultáneamente.
    let _lock = VOICE_LOAD_LOCK.lock().await;
    let dir = voice_dir(&voice.id)
        .ok_or_else(|| "no se pudo resolver config_dir para cachear voces".to_string())?;
    let onnx = dir.join(format!("{}.onnx", voice.id));
    let tokens = dir.join("tokens.txt");
    let espeak_data = dir.join("espeak-ng-data");

    if onnx.exists()
        && tokens.exists()
        && espeak_data.is_dir()
        && std::fs::metadata(&onnx)
            .map(|m| m.len() > 0)
            .unwrap_or(false)
        && std::fs::metadata(&tokens)
            .map(|m| m.len() > 0)
            .unwrap_or(false)
    {
        return Ok(DownloadOutcome::AlreadyPresent);
    }

    // FASE 3 distribución: intentar copiar desde el bundle (offline)
    // antes de descargar de internet.
    std::fs::create_dir_all(&dir).map_err(|e| format!("creando {}: {e}", dir.display()))?;
    if copy_voice_from_bundle(&voice.id, &dir) {
        // Verificar que la copia fue exitosa.
        if onnx.exists() && tokens.exists() && espeak_data.is_dir() {
            info!("[tts] voz '{}' copiada desde el bundle", voice.id);
            return Ok(DownloadOutcome::Downloaded);
        }
        warn!("[tts] copia del bundle incompleta, intentando descarga");
    }

    // Fallback: descargar de internet.
    let tarball = dir.join("voice.tar.bz2");
    info!(
        "[tts] descargando tarball de {} (esto puede tardar ~1 min la primera vez)",
        voice.id
    );
    download_to(voice.tarball_url, &tarball).await?;
    info!("[tts] extrayendo {}", tarball.display());

    // Extrae. Usamos el comando `tar` del sistema porque el formato es
    // `.tar.bz2` y Rust no trae un descompresor bzip2 en stdlib.
    let status = std::process::Command::new("tar")
        .arg("xjf")
        .arg(&tarball)
        .arg("-C")
        .arg(&dir)
        .arg("--strip-components=1")
        .status()
        .map_err(|e| format!("ejecutando tar: {e}"))?;
    if !status.success() {
        return Err(format!(
            "tar falló (exit {:?}) extrayendo {}",
            status.code(),
            tarball.display()
        ));
    }

    // El tarball crea una subcarpeta `vits-piper-<id>/` con el id
    // original (p. ej. `vits-piper-es_ES-davefx-medium/`). Con
    // `--strip-components=1` movemos el contenido al nivel superior,
    // que es lo que espera nuestro layout (`<dir>/<id>.onnx`, etc.).
    // Verificamos.
    if !onnx.exists() {
        return Err(format!(
            "tras extraer no se encontró el .onnx en {}",
            onnx.display()
        ));
    }
    if !tokens.exists() {
        return Err(format!(
            "tras extraer no se encontró el tokens.txt en {}",
            tokens.display()
        ));
    }
    if !espeak_data.is_dir() {
        return Err(format!(
            "tras extraer no se encontró el directorio espeak-ng-data/ en {}",
            espeak_data.display()
        ));
    }

    // Limpia el tarball descargado.
    let _ = std::fs::remove_file(&tarball);
    Ok(DownloadOutcome::Downloaded)
}

/// Descarga un archivo por HTTPS a la ruta indicada (con stream).
async fn download_to(url: &str, dest: &PathBuf) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(600))
        .build()
        .map_err(|e| format!("reqwest client: {e}"))?;

    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("GET {url}: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {} al descargar {url}", resp.status()));
    }

    let mut stream = resp.bytes_stream();
    let mut file = tokio::fs::File::create(dest)
        .await
        .map_err(|e| format!("creando {}: {e}", dest.display()))?;
    let mut total: u64 = 0;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("leyendo stream de {url}: {e}"))?;
        tokio::io::copy_buf(&mut chunk.as_ref(), &mut file)
            .await
            .map_err(|e| format!("escribiendo en {}: {e}", dest.display()))?;
        total += chunk.len() as u64;
    }
    info!("[tts] descargado {} ({} bytes)", dest.display(), total);
    Ok(())
}

// ============================================
// MOTOR TTS
// ============================================

/// Estado del motor TTS.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TtsStatus {
    pub loaded: bool,
    pub voice_id: Option<String>,
    pub model_path: Option<String>,
    pub sample_rate: i32,
    pub num_speakers: i32,
    pub last_error: Option<String>,
}

/// Motor TTS. Es `Send + Sync` porque `OfflineTts` lo es (ver docs del
/// crate). Usamos `AsyncMutex` para serializar la carga inicial y para
/// evitar condiciones de carrera si dos tareas piden cambiar la voz
/// simultáneamente. El método `synthesize` toma samples y los devuelve
/// clonados; la inferencia en sí no es async porque sherpa-onnx
/// expone una API bloqueante.
pub struct TtsEngine {
    inner: Arc<AsyncMutex<TtsInner>>,
}

struct TtsInner {
    /// Voz actualmente cargada (None si nunca se cargó).
    voice_id: Option<String>,
    /// Modelo de sherpa-onnx (None si aún no se cargó). Envuelto en
    /// `Arc` porque `OfflineTts` no implementa `Clone` (solo `Send + Sync`),
    /// y necesitamos poder moverlo al closure de `spawn_blocking`.
    tts: Option<Arc<OfflineTts>>,
    /// Sample rate del modelo actual (para recálculo de duraciones).
    sample_rate: i32,
    /// Último error (si lo hubo) para exponer al frontend.
    last_error: Option<String>,
}

impl TtsEngine {
    /// Crea un motor vacío. No descarga ni carga nada hasta que se
    /// llame a [`set_voice`] o a [`synthesize`].
    pub fn new() -> Self {
        Self {
            inner: Arc::new(AsyncMutex::new(TtsInner {
                voice_id: None,
                tts: None,
                sample_rate: 0,
                last_error: None,
            })),
        }
    }

    /// Estado serializable para el frontend.
    pub async fn status(&self) -> TtsStatus {
        let g = self.inner.lock().await;
        TtsStatus {
            loaded: g.tts.is_some(),
            voice_id: g.voice_id.clone(),
            model_path: g
                .voice_id
                .as_ref()
                .and_then(|v| voice_onnx_path(v))
                .map(|p| p.display().to_string()),
            sample_rate: g.sample_rate,
            num_speakers: g.tts.as_ref().map(|t| t.num_speakers()).unwrap_or(0),
            last_error: g.last_error.clone(),
        }
    }

    /// Cambia la voz activa. Si el modelo no está en disco, lo descarga
    /// (puede tardar ~1 min la primera vez con `davefx/medium` de 63 MB).
    /// Si ya está cargada esa voz, no hace nada.
    pub async fn set_voice(&self, voice_id: &str) -> Result<(), String> {
        let voice = voice_by_id(voice_id)
            .ok_or_else(|| format!("voz desconocida: {voice_id}"))?
            .clone();
        ensure_voice_downloaded(&voice).await?;
        let onnx = voice_onnx_path(&voice.id)
            .ok_or_else(|| "no se pudo resolver ruta del modelo".to_string())?;
        let tokens = voice_tokens_path(&voice.id)
            .ok_or_else(|| "no se pudo resolver ruta de tokens".to_string())?;
        let espeak_data = voice_espeak_data_path(&voice.id)
            .ok_or_else(|| "no se pudo resolver ruta de espeak-ng-data".to_string())?;
        let config = OfflineTtsConfig {
            model: OfflineTtsModelConfig {
                vits: OfflineTtsVitsModelConfig {
                    model: Some(onnx.to_string_lossy().into_owned()),
                    lexicon: None,
                    tokens: Some(tokens.to_string_lossy().into_owned()),
                    data_dir: Some(espeak_data.to_string_lossy().into_owned()),
                    noise_scale: 0.667,
                    noise_scale_w: 0.8,
                    length_scale: 1.0,
                    dict_dir: None,
                },
                ..Default::default()
            },
            rule_fsts: None,
            max_num_sentences: 1,
            rule_fars: None,
            silence_scale: 0.2,
        };
        let tts = OfflineTts::create(&config)
            .ok_or_else(|| format!("no se pudo crear el motor TTS para {}", voice.id))?;
        let sample_rate = tts.sample_rate();
        let mut g = self.inner.lock().await;
        g.voice_id = Some(voice.id.to_string());
        g.tts = Some(Arc::new(tts));
        g.sample_rate = sample_rate;
        g.last_error = None;
        info!(
            "[tts] voz cargada: {} (sample_rate={})",
            voice.label, sample_rate
        );
        Ok(())
    }

    /// Asegura que la voz está cargada; si no, usa la voz por defecto.
    async fn ensure_loaded(&self, voice_id: Option<&str>) -> Result<(), String> {
        let target = voice_id.unwrap_or(DEFAULT_VOICE_ID);
        let needs_load = {
            let g = self.inner.lock().await;
            g.voice_id.as_deref() != Some(target)
        };
        if needs_load {
            self.set_voice(target).await?;
        }
        Ok(())
    }

    /// Sintetiza texto a samples `f32` (mono, sample rate del modelo).
    /// Si no se pasa `voice_id`, usa la voz por defecto.
    ///
    /// **Importante**: `GeneratedAudio` no es `Send` (contiene un puntero
    /// opaco del C-API), así que la inferencia y la copia de samples
    /// se hacen dentro de `spawn_blocking` y solo el `Vec<f32>` cruza
    /// el thread boundary.
    pub async fn synthesize(
        &self,
        text: &str,
        voice_id: Option<&str>,
    ) -> Result<(Vec<f32>, i32), String> {
        if text.trim().is_empty() {
            return Err("el texto a sintetizar está vacío".to_string());
        }
        self.ensure_loaded(voice_id).await?;
        // Clonamos el `Arc<OfflineTts>` para sacarlo del lock durante
        // la inferencia. `OfflineTts` no implementa `Clone`, pero
        // `Arc::clone` es barato y `Send`.
        let (tts_arc, sample_rate) = {
            let g = self.inner.lock().await;
            let tts = g
                .tts
                .as_ref()
                .ok_or_else(|| "motor TTS no cargado".to_string())?;
            (Arc::clone(tts), g.sample_rate)
        };
        let text_owned = text.to_string();
        // `spawn_blocking` exige que el retorno sea `Send + 'static`.
        // `GeneratedAudio` no es `Send`; por eso extraemos los samples
        // a un `Vec<f32>` (que sí es `Send`) dentro del closure,
        // antes de que el `GeneratedAudio` se destruya al final del
        // bloque (su `Drop` libera el C-API).
        let samples: Vec<f32> = tokio::task::spawn_blocking(move || -> Result<Vec<f32>, String> {
            let audio = tts_arc
                .generate_with_config(
                    &text_owned,
                    &sherpa_onnx::GenerationConfig {
                        silence_scale: 0.2,
                        speed: 1.0,
                        sid: 0,
                        reference_audio: None,
                        reference_sample_rate: 0,
                        reference_text: None,
                        num_steps: 0,
                        extra: None,
                    },
                    None::<fn(&[f32], f32) -> bool>,
                )
                .ok_or_else(|| "sherpa-onnx no devolvió audio".to_string())?;
            Ok(audio.samples().to_vec())
        })
        .await
        .map_err(|e| format!("join error: {e}"))??;
        Ok((samples, sample_rate))
    }

    /// Sintetiza y guarda el resultado como WAV en `path`.
    /// Devuelve `(samples, sample_rate)`.
    pub async fn synthesize_to_wav(
        &self,
        text: &str,
        voice_id: Option<&str>,
        path: &str,
    ) -> Result<(usize, i32), String> {
        if text.trim().is_empty() {
            return Err("el texto a sintetizar está vacío".to_string());
        }
        self.ensure_loaded(voice_id).await?;
        // Mismo truco: `Arc::clone` para mover al closure.
        let (tts_arc, sample_rate) = {
            let g = self.inner.lock().await;
            let tts = g
                .tts
                .as_ref()
                .ok_or_else(|| "motor TTS no cargado".to_string())?;
            (Arc::clone(tts), g.sample_rate)
        };
        let text_owned = text.to_string();
        let path_owned = path.to_string();
        // Mismo truco: hacer el `save` (que toma `&str`) y la copia
        // de samples dentro del closure para que solo el `Vec<f32>`
        // cruce el thread boundary. `save` recibe el path por valor
        // pero `&str` es copia barata.
        let n = tokio::task::spawn_blocking(move || -> Result<usize, String> {
            let audio = tts_arc
                .generate_with_config(
                    &text_owned,
                    &sherpa_onnx::GenerationConfig {
                        silence_scale: 0.2,
                        speed: 1.0,
                        sid: 0,
                        reference_audio: None,
                        reference_sample_rate: 0,
                        reference_text: None,
                        num_steps: 0,
                        extra: None,
                    },
                    None::<fn(&[f32], f32) -> bool>,
                )
                .ok_or_else(|| "sherpa-onnx no devolvió audio".to_string())?;
            let n = audio.samples().len();
            if !audio.save(&path_owned) {
                return Err(format!("no se pudo escribir el WAV en {path_owned}"));
            }
            Ok(n)
        })
        .await
        .map_err(|e| format!("join error: {e}"))??;
        Ok((n, sample_rate))
    }
}

// ============================================
// WAV EN MEMORIA (para enviar al frontend como base64)
// ============================================

/// Cabecera mínima de un WAV PCM 16-bit mono, escrita a mano para
/// evitar la dependencia `hound`. Devuelve los bytes del archivo
/// WAV completo (cabecera + samples).
///
/// `samples` son f32 en rango [-1.0, 1.0]. Se clampean a [-1.0, 1.0]
/// y se cuantizan a i16.
pub fn samples_f32_to_wav_bytes(samples: &[f32], sample_rate: i32) -> Vec<u8> {
    let num_samples = samples.len();
    let byte_rate = (sample_rate as u32) * 2; // 16-bit mono = 2 bytes/sample
    let block_align: u16 = 2;
    let data_size: u32 = (num_samples as u32) * 2;
    let chunk_size: u32 = 36 + data_size;

    let mut out = Vec::with_capacity(44 + data_size as usize);
    // RIFF header
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&chunk_size.to_le_bytes());
    out.extend_from_slice(b"WAVE");
    // fmt subchunk
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes()); // PCM fmt chunk size
    out.extend_from_slice(&1u16.to_le_bytes()); // audio format = PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // num channels = 1 (mono)
    out.extend_from_slice(&(sample_rate as u32).to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
                                                 // data subchunk
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_size.to_le_bytes());
    // samples
    for &s in samples {
        let clamped = s.clamp(-1.0, 1.0);
        let i = (clamped * 32767.0) as i16;
        out.extend_from_slice(&i.to_le_bytes());
    }
    out
}

impl Default for TtsEngine {
    fn default() -> Self {
        Self::new()
    }
}
