//! STT local open source (FASE 2.4.C).
//!
//! Wrapper alrededor de `sherpa-onnx` para reconocimiento de voz en
//! streaming (OnlineRecognizer). Usa el modelo Zipformer-Transducer
//! `sherpa-onnx-streaming-zipformer-en` (k2-fsa, 2023), que es multilingüe
//! y es el ÚNICO modelo Zipformer streaming estable publicado oficialmente
//! por k2-fsa a junio 2026. La captura de audio del micrófono se hace con
//! `cpal` (ver el comando `stt_start` en `lib.rs`).
//!
//! ## Modelo
//!
//! - ID: `sherpa-onnx-streaming-zipformer-en` (k2-fsa, 2023, multilingüe).
//! - Tamaño: ~310 MB en tarball.
//! - Idioma: aunque el id dice `-en`, es multilingüe y transcribe español
//!   razonablemente bien. No hay un modelo streaming Zipformer específico
//!   para español en los releases oficiales de k2-fsa a fecha de hoy.
//! - Salida: transcripción parcial cada N ms y final al detectar
//!   fin de utterance (endpoint).
//!
//! ## Notas de implementación
//!
//! - El modelo se carga perezoso: la primera vez que se invoca `set_voice`
//!   se descarga y se cachea en `~/.config/synapse-cortana/voices/<id>/`.
//! - El reconocedor es `Send + Sync` pero NO clonable: lo guardamos
//!   detrás de `Mutex` (no `AsyncMutex`) porque `accept_waveform` es
//!   síncrona y rápida.
//! - El endpoint detection (`enable_endpoint = true`) emite
//!   `is_endpoint = true` cuando detecta silencio al final de una
//!   frase, lo que usamos para sugerir al frontend que detenga la
//!   grabación.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use sherpa_onnx::{
    OfflineRecognizer, OfflineRecognizerConfig, OnlineRecognizer, OnlineRecognizerConfig,
};

use tokio::sync::Mutex as AsyncMutex;

// ============================================
// ESPECIFICACIONES DE MODELOS STT
// ============================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SttModelSpec {
    /// ID estable. Se usa como nombre de carpeta en disco y para
    /// `tts_set_voice`. Ej.: `sherpa-onnx-streaming-zipformer-es`.
    pub id: &'static str,
    /// Etiqueta legible para mostrar en la UI.
    pub label: &'static str,
    /// Locale BCP-47.
    pub locale: &'static str,
    /// URL del tarball oficial de k2-fsa (igual estilo que TTS).
    pub tarball_url: &'static str,
    /// Tamaño aproximado en MB.
    pub size_mb_approx: u32,
}

/// Catálogo de modelos STT disponibles.
///
/// Orden importa: el primer modelo es el **por defecto** (streaming, baja
/// latencia, multilingüe pero entrenado principalmente en inglés). El
/// segundo y tercero son **offline** Whisper (más preciso, multilingüe
/// nativo incluyendo español) — el usuario puede elegir desde la UI si
/// quiere dictado en español fiable.
///
/// 1. **`sherpa-onnx-streaming-zipformer-en`** (junio 2023, k2-fsa):
///    streaming Zipformer Transducer, ~310 MB, baja latencia (<300 ms).
///    El id dice `-en` pero el modelo es multilingüe y transcribe
///    razonablemente bien frases cortas en español. Ideal para dictado
///    en tiempo real.
///
/// 2. **`sherpa-onnx-whisper-tiny`** (multilingüe, OpenAI Whisper tiny):
///    offline, no streaming, multilingüe nativo (99 idiomas), entiende
///    español razonablemente bien. ~116 MB. Latencia mayor (~1-3 s por
///    utterance) porque procesa el audio completo. **Transcripción
///    ocasionalmente imperfecta** (ej. "Alan" → "Dalán") por ser el
///    modelo más pequeño.
///
/// 3. **`sherpa-onnx-whisper-base`** (multilingüe, OpenAI Whisper base):
///    igual que tiny pero con **mayor precisión** en español (modelo
///    ~150 MB). **Recomendado para dictado en español** cuando la
///    latencia no es prioritaria.
pub const STT_MODEL_CATALOG: &[SttModelSpec] = &[
    SttModelSpec {
        id: "sherpa-onnx-whisper-medium",
        label: "Whisper medium multilingüe (máxima calidad, ~900 MB)",
        locale: "multi",
        tarball_url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-whisper-medium.tar.bz2",
        size_mb_approx: 900,
    },
    SttModelSpec {
        id: "sherpa-onnx-whisper-base",
        label: "Whisper base multilingüe (más rápido, ~150 MB)",
        locale: "multi",
        tarball_url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-whisper-base.tar.bz2",
        size_mb_approx: 150,
    },
];

/// Por defecto usamos Whisper medium porque es el que empaquetamos en el
/// bundle (máxima calidad para dictado en español).
pub const DEFAULT_STT_MODEL_ID: &str = "sherpa-onnx-whisper-medium";

pub fn stt_model_by_id(id: &str) -> Option<&'static SttModelSpec> {
    STT_MODEL_CATALOG.iter().find(|m| m.id == id)
}

// ============================================
// RUTAS EN DISCO
// ============================================

fn stt_models_root() -> Option<PathBuf> {
    let base = directories::ProjectDirs::from("ai", "openclaw", "synapse-cortana")?;
    Some(base.config_dir().join("stt-models"))
}

pub fn stt_model_dir(model_id: &str) -> Option<PathBuf> {
    Some(stt_models_root()?.join(model_id))
}

/// Directorio donde Tauri empaqueta los recursos del bundle.
static BUNDLE_RESOURCE_DIR: std::sync::OnceLock<Option<PathBuf>> = std::sync::OnceLock::new();

/// Inicializa el directorio de recursos del bundle.
pub fn init_bundle_resources(path: PathBuf) {
    let _ = BUNDLE_RESOURCE_DIR.set(Some(path));
}

/// Copia un directorio recursivamente.
fn copy_dir_recursive(src: &Path, dst: &Path) -> bool {
    if let Err(e) = std::fs::create_dir_all(dst) {
        warn!("[stt] crear dir {}: {e}", dst.display());
        return false;
    }
    let entries = match std::fs::read_dir(src) {
        Ok(e) => e,
        Err(e) => {
            warn!("[stt] read_dir {}: {e}", src.display());
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
                warn!("[stt] copiar {} → {}: {e}", from.display(), to.display());
                return false;
            }
        }
    }
    true
}

/// Busca un modelo STT en el bundle y lo copia a ~/.config/.
fn copy_stt_from_bundle(model_id: &str, target_dir: &Path) -> bool {
    let Some(bundle_dir) = BUNDLE_RESOURCE_DIR.get() else {
        return false;
    };
    let Some(bundle_path) = bundle_dir else {
        return false;
    };
    let src = bundle_path
        .join("resources")
        .join("stt-models")
        .join(model_id);
    if !src.exists() {
        return false;
    }
    info!(
        "[stt] copiando modelo '{}' desde el bundle (offline)",
        model_id
    );
    copy_dir_recursive(&src, target_dir)
}

// ============================================
// DESCARGA DE MODELO
// ============================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadOutcome {
    AlreadyPresent,
    Downloaded,
}

/// Asegura que el modelo STT está descargado. Devuelve `Downloaded` si
/// hubo que descargarlo o `AlreadyPresent` si ya estaba.
pub async fn ensure_stt_model_downloaded(model_id: &str) -> Result<DownloadOutcome, String> {
    let dir =
        stt_model_dir(model_id).ok_or_else(|| "no se pudo resolver stt_models_root".to_string())?;
    // Si ya existe el archivo de tokens (parte del modelo), asumimos
    // que está completo. Soportamos:
    //   - `tokens.txt` (streaming Zipformer, modelos más viejos)
    //   - `tiny-tokens.txt` (Whisper tiny)
    //   - `base-tokens.txt` (Whisper base)
    //   - cualquier archivo que termine en `-tokens.txt` como fallback.
    let tokens_already_present = dir.join("tokens.txt").exists()
        || dir.join("tiny-tokens.txt").exists()
        || dir.join("base-tokens.txt").exists()
        || std::fs::read_dir(&dir)
            .ok()
            .map(|entries| {
                entries.flatten().any(|e| {
                    e.file_name()
                        .to_str()
                        .map(|n| n.ends_with("-tokens.txt"))
                        .unwrap_or(false)
                })
            })
            .unwrap_or(false);
    if tokens_already_present && find_any_onnx_in_dir(&dir) {
        return Ok(DownloadOutcome::AlreadyPresent);
    }

    // FASE 3 distribución: intentar copiar desde el bundle (offline)
    // antes de descargar de internet.
    std::fs::create_dir_all(&dir).map_err(|e| format!("crear {}: {e}", dir.display()))?;
    if copy_stt_from_bundle(model_id, &dir) {
        if find_any_onnx_in_dir(&dir) {
            info!("[stt] modelo '{}' copiado desde el bundle", model_id);
            return Ok(DownloadOutcome::Downloaded);
        }
        warn!("[stt] copia del bundle incompleta, intentando descarga");
    }

    let spec =
        stt_model_by_id(model_id).ok_or_else(|| format!("modelo STT desconocido: {model_id}"))?;

    info!(
        "[stt] descargando modelo {} desde {}",
        spec.id, spec.tarball_url
    );
    std::fs::create_dir_all(&dir).map_err(|e| format!("crear {}: {e}", dir.display()))?;

    // Limpiar cualquier extracción previa corrupta (subcarpetas con archivos
    // sueltos de un intento de descarga anterior que falló a mitad).
    clean_dir_before_extract(&dir);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(600))
        .build()
        .map_err(|e| format!("reqwest: {e}"))?;
    let resp = client
        .get(spec.tarball_url)
        .send()
        .await
        .map_err(|e| format!("GET {}: {e}", spec.tarball_url))?;
    if !resp.status().is_success() {
        return Err(format!("descarga STT falló: HTTP {}", resp.status()));
    }
    let bytes = resp.bytes().await.map_err(|e| format!("leer bytes: {e}"))?;
    let tarball_path = dir.join("model.tar.bz2");
    std::fs::write(&tarball_path, &bytes).map_err(|e| format!("escribir tarball: {e}"))?;

    // Extraer tar.bz2 en `dir`. Usamos `tar` del sistema (Linux/macOS).
    let status = std::process::Command::new("tar")
        .arg("-xjf")
        .arg(&tarball_path)
        .arg("-C")
        .arg(&dir)
        .status()
        .map_err(|e| format!("ejecutar `tar -xjf` (¿está instalado?): {e}"))?;
    if !status.success() {
        // Limpiar el tarball corrupto para que el próximo intento
        // descargue de nuevo.
        let _ = std::fs::remove_file(&tarball_path);
        return Err(format!("tar salió con {}", status));
    }
    let _ = std::fs::remove_file(&tarball_path);

    // El tarball oficial de k2-fsa pone los archivos en una subcarpeta
    // con el nombre del modelo. Los movemos al directorio padre (`dir`)
    // para que la búsqueda simple `dir.join("*.onnx")` funcione.
    flatten_model_dir(&dir).map_err(|e| format!("aplanar modelo: {e}"))?;

    Ok(DownloadOutcome::Downloaded)
}

/// `true` si el directorio contiene al menos un archivo `.onnx` (parte
/// esencial del modelo STT). Lo usamos para decidir si el modelo ya está
/// descargado (algunos tarballs antiguos solo traen `tokens.txt` y otros
/// archivos auxiliares, así que verificar SOLO `tokens.txt` no es
/// suficiente).
fn find_any_onnx_in_dir(dir: &std::path::Path) -> bool {
    std::fs::read_dir(dir)
        .ok()
        .map(|entries| {
            entries.flatten().any(|e| {
                e.path()
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| ext == "onnx")
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

/// Limpia archivos `.tar.bz2`, archivos `.onnx` parciales, y subcarpetas
/// sospechosas dentro de `dir` antes de extraer un nuevo tarball. Esto
/// evita el error `tar: Unerwartetes Dateiende im Archiv` cuando una
/// descarga anterior dejó archivos a medias.
fn clean_dir_before_extract(dir: &std::path::Path) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let p = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if p.is_file() {
            // Borrar tarballs viejos.
            if name.ends_with(".tar.bz2") || name.ends_with(".tar") {
                info!("[stt] borrando tarball previo: {}", name);
                let _ = std::fs::remove_file(&p);
            }
        } else if p.is_dir() {
            // Si la subcarpeta tiene un tarball o ya tiene archivos de
            // modelo (.onnx, tokens.txt), la dejamos. Si está vacía o
            // tiene archivos sueltos de una descarga corrupta previa,
            // la borramos para que `tar` pueda crear la nueva sin
            // chocar con archivos viejos.
            let inner_entries = match std::fs::read_dir(&p) {
                Ok(e) => e,
                Err(_) => continue,
            };
            let inner: Vec<_> = inner_entries.flatten().collect();
            if inner.is_empty() {
                let _ = std::fs::remove_dir(&p);
            } else {
                // Si la subcarpeta ya tiene un tarball parcial, la borramos.
                let has_partial_tar = inner.iter().any(|e| {
                    e.file_name()
                        .to_str()
                        .map(|n| n.ends_with(".tar.bz2") || n.ends_with(".tar"))
                        .unwrap_or(false)
                });
                if has_partial_tar {
                    info!("[stt] borrando subcarpeta con tarball parcial: {}", name);
                    let _ = std::fs::remove_dir_all(&p);
                }
            }
        }
    }
}

/// Si `dir` contiene exactamente una subcarpeta con los archivos del
/// modelo (`*.onnx`, `*tokens.txt`), mueve su contenido al nivel
/// superior de `dir` y borra la subcarpeta.
fn flatten_model_dir(dir: &std::path::Path) -> Result<(), String> {
    // Si ya están los archivos esperados a nivel raíz, no hacemos nada.
    if find_any_onnx_in_dir(dir)
        && (dir.join("tokens.txt").exists()
            || dir.join("tiny-tokens.txt").exists()
            || dir.join("base-tokens.txt").exists())
    {
        return Ok(());
    }
    let entries = std::fs::read_dir(dir).map_err(|e| format!("read_dir: {e}"))?;
    let mut subdirs = Vec::new();
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            subdirs.push(p);
        }
    }
    // Si hay exactamente una subcarpeta, aplanarla.
    if subdirs.len() == 1 {
        let sub = &subdirs[0];
        // Verificar que la subcarpeta parece un modelo STT (tiene al
        // menos un .onnx y un *tokens.txt).
        let has_onnx = find_any_onnx_in_dir(sub);
        let has_tokens = sub.join("tokens.txt").exists()
            || sub.join("tiny-tokens.txt").exists()
            || sub.join("base-tokens.txt").exists();
        if has_onnx && has_tokens {
            info!("[stt] aplanando subcarpeta {}", sub.display());
            for entry in std::fs::read_dir(sub)
                .map_err(|e| format!("read_dir sub: {e}"))?
                .flatten()
            {
                let from = entry.path();
                let to = dir.join(entry.file_name());
                // Si ya existe el destino (por una descarga corrupta
                // previa), sobrescribirlo.
                if to.exists() {
                    let _ = std::fs::remove_file(&to);
                }
                std::fs::rename(&from, &to).map_err(|e| format!("rename: {e}"))?;
            }
            std::fs::remove_dir(sub).map_err(|e| format!("rmdir: {e}"))?;
        } else {
            info!(
                "[stt] subcarpeta {} ignorada (has_onnx={}, has_tokens={})",
                sub.display(),
                has_onnx,
                has_tokens
            );
        }
    }
    Ok(())
}

/// Busca un archivo del modelo por prefijo. Ej: `find_model_file(dir, "encoder")`
/// devuelve la ruta a `encoder-epoch-99-avg-1-chunk-16-left-128.onnx` (no int8).
/// Prefiere la versión NO cuantizada (sin `.int8.`) porque sherpa-onnx
/// no soporta cuantización con esta API.
fn find_model_file(dir: &std::path::Path, prefix: &str) -> Option<std::path::PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    let mut non_int8 = None;
    for entry in entries.flatten() {
        let p = entry.path();
        if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
            if name.starts_with(prefix) && name.ends_with(".onnx") {
                if name.contains(".int8.") {
                    // Lo guardamos como fallback pero seguimos buscando.
                    if non_int8.is_none() {
                        non_int8 = Some(p.clone());
                    }
                } else {
                    return Some(p);
                }
            }
        }
    }
    non_int8
}

// ============================================
// MOTOR STT
// ============================================

/// Tipo de motor STT. El streaming Zipformer es el de baja latencia
/// (OnlineRecognizer). Whisper tiny es offline (OfflineRecognizer) y
/// procesa el audio completo al final.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SttEngineKind {
    StreamingZipformer,
    OfflineWhisper,
}

#[derive(Clone, Debug, Serialize)]
pub struct SttStatus {
    pub loaded: bool,
    pub model_id: String,
    pub model_path: Option<String>,
    pub sample_rate: u32,
    pub engine_kind: Option<SttEngineKind>,
    pub last_error: Option<String>,
}

/// Estado interno de un motor STT. El `engine_kind` decide cómo se
/// procesa el audio en `lib.rs::stt_start`.
///
/// `OnlineRecognizer` y `OfflineRecognizer` NO implementan `Clone`
/// (guardan punteros crudos de la librería C de sherpa-onnx). Por eso
/// el acceso se hace a través de `SttEngine` que es clonable.
///
/// `OfflineRecognizer` SÍ implementa `Send + Sync` (es unsafe-impl), así
/// que podemos envolverlo en `Arc` y compartirlo entre el `stt_set_model`
/// y el `stt_start` sin necesidad de recrearlo cada vez (ahorra ~500 ms
/// de latencia en el primer 🎙️).
struct SttInner {
    model_id: String,
    engine_kind: SttEngineKind,
    /// Streaming Zipformer Transducer (latencia <300 ms).
    online: Option<OnlineRecognizer>,
    online_config: Option<OnlineRecognizerConfig>,
    /// Offline Whisper (latencia ~1-3 s por utterance, multilingüe nativo).
    /// **Cacheado como `Arc`** para evitar recrearlo en cada `stt_start`.
    offline_arc: Option<Arc<OfflineRecognizer>>,
    offline_config: Option<OfflineRecognizerConfig>,
    sample_rate: u32,
}

/// Snapshot ligero del estado del motor STT. **No clona los recognizers**
/// (no son `Clone`); el hilo de reconocimiento accede al motor vía
/// `inner_arc` (`Arc<AsyncMutex<Option<SttInner>>>`) que es clonable y
/// compartido entre la GUI y el hilo de audio.
#[derive(Clone)]
pub struct RecognizerHandle {
    pub model_id: String,
    pub engine_kind: SttEngineKind,
    pub sample_rate: u32,
    /// `Arc` al lock interno del `SttEngine`. Permite al hilo de audio
    /// clonar el recognizer cuando necesita un nuevo stream.
    pub inner_arc: Arc<AsyncMutex<Option<SttInner>>>,
}

impl RecognizerHandle {
    /// Versión **síncrona** de `online_recognizer_clone`. Útil para hilos
    /// que no pueden ser async (como el closure de `std::thread::spawn`
    /// que crea el stream de audio). Bloquea el lock con `blocking_lock`.
    pub fn online_recognizer_clone_blocking(&self) -> Option<OnlineRecognizer> {
        let guard = self.inner_arc.blocking_lock();
        let cfg = guard.as_ref()?.online_config.clone()?;
        OnlineRecognizer::create(&cfg)
    }
    /// Devuelve un clon del `Arc<OfflineRecognizer>` cacheado. Esto
    /// evita recrear el recognizer en cada `stt_start` (ahorra ~500 ms).
    pub fn offline_recognizer_arc_blocking(&self) -> Option<Arc<OfflineRecognizer>> {
        let guard = self.inner_arc.blocking_lock();
        guard.as_ref()?.offline_arc.clone()
    }
    /// Fallback: crea un `OfflineRecognizer` nuevo desde la config.
    pub fn offline_recognizer_clone_blocking(&self) -> Option<OfflineRecognizer> {
        let guard = self.inner_arc.blocking_lock();
        let cfg = guard.as_ref()?.offline_config.clone()?;
        OfflineRecognizer::create(&cfg)
    }
    /// Versión async (preferida cuando el caller puede esperar).
    pub async fn online_recognizer_clone(&self) -> Option<OnlineRecognizer> {
        let guard = self.inner_arc.lock().await;
        let cfg = guard.as_ref()?.online_config.clone()?;
        OnlineRecognizer::create(&cfg)
    }
    pub async fn offline_recognizer_clone(&self) -> Option<OfflineRecognizer> {
        let guard = self.inner_arc.lock().await;
        let cfg = guard.as_ref()?.offline_config.clone()?;
        OfflineRecognizer::create(&cfg)
    }
}

pub struct SttEngine {
    inner: Arc<AsyncMutex<Option<SttInner>>>,
    status: Arc<AsyncMutex<SttStatus>>,
}

impl SttEngine {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(AsyncMutex::new(None)),
            status: Arc::new(AsyncMutex::new(SttStatus {
                loaded: false,
                model_id: String::new(),
                model_path: None,
                sample_rate: 16000,
                engine_kind: None,
                last_error: None,
            })),
        }
    }

    pub async fn status(&self) -> SttStatus {
        self.status.lock().await.clone()
    }

    /// Devuelve un `Arc` al lock interno para que `lib.rs` pueda
    /// acceder al recognizer sin clonarlo. Usar junto con
    /// `recognizer_clone()` cuando sea estrictamente necesario.
    pub fn inner_arc(&self) -> Arc<AsyncMutex<Option<SttInner>>> {
        self.inner.clone()
    }

    /// Devuelve el `RecognizerHandle` que `lib.rs` necesita para
    /// crear streams / invocar el decoder.
    pub async fn handle(&self) -> Option<RecognizerHandle> {
        let guard = self.inner.lock().await;
        let inner = guard.as_ref()?;
        Some(RecognizerHandle {
            model_id: inner.model_id.clone(),
            engine_kind: inner.engine_kind,
            sample_rate: inner.sample_rate,
            inner_arc: self.inner.clone(),
        })
    }

    /// Carga el modelo STT si no está cargado. Equivalente a `tts_set_voice`.
    /// Detecta automáticamente el motor según el `model_id`:
    ///   - `sherpa-onnx-streaming-zipformer-en` → `OnlineRecognizer` (streaming).
    ///   - `sherpa-onnx-whisper-tiny` → `OfflineRecognizer` (Whisper).
    pub async fn set_model(&self, model_id: &str) -> Result<SttStatus, String> {
        let spec = stt_model_by_id(model_id)
            .ok_or_else(|| format!("modelo STT desconocido: {model_id}"))?;

        // Descargar si hace falta.
        ensure_stt_model_downloaded(model_id).await?;

        let dir = stt_model_dir(model_id)
            .ok_or_else(|| "no se pudo resolver stt_model_dir".to_string())?;

        // Despachar por tipo de motor.
        let (engine_kind, online, online_config, offline_arc, offline_config, sample_rate) =
            if model_id == "sherpa-onnx-whisper-tiny" || model_id == "sherpa-onnx-whisper-base" {
                let (rec, cfg) = build_offline_whisper(&dir, model_id)?;
                // Cachear el recognizer en `Arc` para que el primer
                // `stt_start` no tenga que crearlo (ahorra ~500 ms).
                let offline_arc = Some(Arc::new(rec));
                (
                    SttEngineKind::OfflineWhisper,
                    None,
                    None,
                    offline_arc,
                    Some(cfg),
                    16000,
                )
            } else {
                // Por defecto: streaming Zipformer.
                let (rec, cfg) = build_online_zipformer(&dir)?;
                (
                    SttEngineKind::StreamingZipformer,
                    Some(rec),
                    Some(cfg),
                    None,
                    None,
                    16000,
                )
            };

        let inner = SttInner {
            model_id: model_id.to_string(),
            engine_kind,
            online,
            online_config,
            offline_arc,
            offline_config,
            sample_rate,
        };
        let new_status = SttStatus {
            loaded: true,
            model_id: model_id.to_string(),
            model_path: Some(dir.to_string_lossy().to_string()),
            sample_rate,
            engine_kind: Some(engine_kind),
            last_error: None,
        };
        *self.inner.lock().await = Some(inner);
        *self.status.lock().await = new_status.clone();
        Ok(new_status)
    }
}

impl Default for SttEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Construye un `OnlineRecognizer` para el modelo streaming Zipformer
/// Transducer (encoder + decoder + joiner + tokens en el dir).
fn build_online_zipformer(
    dir: &std::path::Path,
) -> Result<(OnlineRecognizer, OnlineRecognizerConfig), String> {
    let tokens = dir.join("tokens.txt");
    let encoder = find_model_file(dir, "encoder")
        .ok_or_else(|| format!("falta encoder-*.onnx en {}", dir.display()))?;
    let decoder = find_model_file(dir, "decoder")
        .ok_or_else(|| format!("falta decoder-*.onnx en {}", dir.display()))?;
    let joiner = find_model_file(dir, "joiner")
        .ok_or_else(|| format!("falta joiner-*.onnx en {}", dir.display()))?;
    if !tokens.exists() {
        return Err(format!("falta tokens.txt en {}", dir.display()));
    }

    let mut config = OnlineRecognizerConfig::default();
    config.feat_config.sample_rate = 16000;
    config.feat_config.feature_dim = 80;
    config.model_config.transducer.encoder = Some(encoder.to_string_lossy().to_string());
    config.model_config.transducer.decoder = Some(decoder.to_string_lossy().to_string());
    config.model_config.transducer.joiner = Some(joiner.to_string_lossy().to_string());
    config.model_config.tokens = Some(tokens.to_string_lossy().to_string());
    config.model_config.num_threads = 2;
    // NO seteamos `model_type` ni `provider` porque el doc-comment oficial
    // de sherpa-onnx 1.13 para streaming Zipformer transducer solo
    // requiere encoder/decoder/joiner/tokens. Forzar `model_type = "zipformer2"`
    // o `provider = "cpu"` rompe la inferencia (el modelo carga pero no emite texto).
    config.decoding_method = Some("greedy_search".to_string());
    config.enable_endpoint = true;

    let recognizer = OnlineRecognizer::create(&config)
        .ok_or_else(|| "OnlineRecognizer::create devolvió null".to_string())?;
    Ok((recognizer, config))
}

/// Construye un `OfflineRecognizer` para Whisper (tiny o base).
/// El tarball oficial trae:
///   - `tiny-encoder.onnx` / `base-encoder.onnx`
///   - `tiny-decoder.onnx` / `base-decoder.onnx`
///   - `tiny-tokens.txt` / `base-tokens.txt`
fn build_offline_whisper(
    dir: &std::path::Path,
    model_id: &str,
) -> Result<(OfflineRecognizer, OfflineRecognizerConfig), String> {
    // Determinar el prefijo de archivo según el modelo.
    let prefix = if model_id.contains("medium") {
        "medium"
    } else if model_id.contains("base") {
        "base"
    } else if model_id.contains("tiny") {
        "tiny"
    } else {
        // Fallback: intentar con "base".
        "base"
    };
    // Whisper usa `{prefix}-tokens.txt` en vez de `tokens.txt`.
    let tokens = if dir.join(format!("{prefix}-tokens.txt")).exists() {
        dir.join(format!("{prefix}-tokens.txt"))
    } else {
        dir.join("tokens.txt")
    };
    let encoder = find_model_file(dir, "encoder")
        .or_else(|| find_model_file(dir, &format!("{prefix}-encoder")))
        .ok_or_else(|| format!("falta encoder-*.onnx en {}", dir.display()))?;
    let decoder = find_model_file(dir, "decoder")
        .or_else(|| find_model_file(dir, &format!("{prefix}-decoder")))
        .ok_or_else(|| format!("falta decoder-*.onnx en {}", dir.display()))?;
    if !tokens.exists() {
        return Err(format!(
            "falta {prefix}-tokens.txt (o tokens.txt) en {}",
            dir.display()
        ));
    }

    let mut config = OfflineRecognizerConfig::default();
    config.feat_config.sample_rate = 16000;
    config.feat_config.feature_dim = 80;
    config.model_config.whisper.encoder = Some(encoder.to_string_lossy().to_string());
    config.model_config.whisper.decoder = Some(decoder.to_string_lossy().to_string());
    config.model_config.tokens = Some(tokens.to_string_lossy().to_string());
    config.model_config.model_type = Some("whisper".to_string());
    config.model_config.num_threads = 2;
    // Detección de idioma: usamos "es" por defecto (configurable más
    // adelante). Whisper tiny/base aceptan códigos como "es", "en",
    // "auto" NO es válido (whisper-greedy-search-decoder.cc lo rechaza).
    config.model_config.whisper.language = Some("es".to_string());
    config.model_config.whisper.task = Some("transcribe".to_string());
    config.model_config.whisper.tail_paddings = 1000;

    let recognizer = OfflineRecognizer::create(&config)
        .ok_or_else(|| "OfflineRecognizer::create devolvió null".to_string())?;
    Ok((recognizer, config))
}
