// SynapseCortana - FASE 1: El Cascarón Conectivo
// Conexión WebSocket con OpenClaw según el protocolo oficial v4.
//
// Documentación del protocolo:
//   - Transporte: WebSocket, tramas JSON de texto.
//   - Handshake: el servidor envía un evento `connect.challenge` con
//     { nonce, ts }. El cliente responde con una solicitud `connect`
//     que incluye `params.auth.token` (cuando hay secreto compartido) y
//     la firma del nonce si se requiere identidad de dispositivo.
//   - Después de un `connect` aceptado, el servidor envía una respuesta
//     `res` con `payload.type == "hello-ok"`.
//   - Las llamadas a RPC posteriores son `{ type: "req", id, method, params }`
//     y vuelven como `{ type: "res", id, ok, payload | error }`.
//
// Referencia:
//   - https://docs.openclaw.ai/es/gateway/protocol
//
// Notas de diseño de esta FASE 1:
//   * Soportamos los modos de autenticación "loopback backend" y
//     "secreto compartido" (`auth.token`). No firmamos el nonce con
//     clave de dispositivo: si el gateway exige device-auth, devolvemos
//     un error claro y dejamos la conexión cerrada.
//   * El stream WebSocket vivo se divide en dos mitades: el `Sink`
//     (escritura) se guarda en `AppState` para enviar RPCs; la mitad
//     `Stream` (lectura) la consume una tarea en background que llena
//     un `inbox` consultable desde el frontend vía `poll_gateway_events`.
//   * `AppState` es clonable (todos sus campos son `Arc<...>`) para que
//     las tareas en background puedan poseer su propia copia.
//   * `chat.send` se envía como `req` con método `chat.send`; los
//     eventos `chat` y `agent` que lleguen se persisten en un buffer
//     acotado.
//
// Device identity:
//   * OpenClaw v2026.6.6 exige que **todas** las conexiones firmen el
//     `connect.challenge` nonce con una clave Ed25519 persistente.
//   * Generamos (o cargamos) el par de claves la primera vez y lo
//     guardamos en `~/.config/synapse-cortana/device.key` (PKCS8 PEM).
//   * `device.id` = SHA-256(publicKey_der) en hex minúsculas.
//   * Firmamos la carga v3 que incluye `platform`, `deviceFamily`,
//     `client`, `role`, `scopes`, `token`, `nonce`. La firma v2 también
//     se acepta, pero preferimos v3.
//
// Referencia:
//   - https://docs.openclaw.ai/es/gateway/protocol

use base64::Engine as _;
use ed25519_dalek::{Signature, Signer, SigningKey};
use futures_util::stream::SplitSink;
use futures_util::{SinkExt, StreamExt};
use log::{debug, error, info, warn};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sherpa_onnx::OnlineRecognizer;
use std::path::PathBuf;
use std::ptr;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, State};
use tokio::net::TcpStream;
use tokio::sync::Mutex as AsyncMutex;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, http::HeaderValue, Message},
    MaybeTlsStream, WebSocketStream,
};

pub mod tts;
pub use tts::{TtsEngine, TtsStatus, VoiceSpec, VOICE_CATALOG};

pub mod stt;
pub use stt::{
    SttEngine, SttEngineKind, SttModelSpec, SttStatus, DEFAULT_STT_MODEL_ID, STT_MODEL_CATALOG,
};

// ============================================
// SETTINGS PERSISTENTES (FASE 2.4)
// ============================================

/// Configuración persistente del usuario. Se serializa a JSON en
/// `~/.config/synapse-cortana/settings.json` y se rehidrata en cada
/// arranque. El token del gateway se guarda aquí (no en `device.key`)
/// y el archivo se crea con permisos `0600` para que no sea legible
/// por otros usuarios del sistema.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppSettings {
    /// URL HTTP del gateway de OpenClaw.
    #[serde(default = "default_gateway_url")]
    pub gateway_url: String,
    /// Token compartido para autenticarse con el gateway.
    #[serde(default)]
    pub gateway_token: String,
    /// ID de la voz TTS seleccionada (ver `VOICE_CATALOG`).
    #[serde(default = "default_voice_id")]
    pub voice_id: String,
    /// Si es `true`, las respuestas del LLM se sintetizan con TTS.
    #[serde(default = "default_true")]
    pub auto_speak: bool,
    /// SessionKey usada en `chat.send`. Default `agent:main:main`.
    #[serde(default = "default_session_key")]
    pub session_key: String,
    /// Última pestaña activa (`"config"` o `"chat"`). Persistida para
    /// que la app recuerde dónde la dejó el usuario.
    #[serde(default = "default_last_tab")]
    pub last_tab: String,
    /// ID del modelo STT (para dictado por voz, FASE 2.4.C).
    /// Vacío = sin modelo STT instalado todavía.
    #[serde(default)]
    pub stt_model_id: String,
    /// Si es `true`, al terminar de dictar se envía el mensaje sin
    /// requerir confirmación del usuario.
    #[serde(default)]
    pub auto_send_after_dictation: bool,
    /// FASE 2.5: tiempo de espera sin nuevos chunks de streaming para
    /// considerar que la respuesta del LLM terminó (en milisegundos).
    /// Más bajo = respuesta más rápida pero puede cortar frases en LLMs
    /// lentos. Más alto = más robusto pero espera más. Default 1500 ms.
    #[serde(default = "default_silence_timeout_ms")]
    pub silence_timeout_ms: u64,
    /// FASE 2.5: timeout global máximo para `chat_and_speak` (en ms).
    /// Si pasan estos ms sin recibir `chat.done`/`agent.done`, devuelve
    /// lo que tenga hasta ese momento. Default 30000 ms (30 s).
    #[serde(default = "default_overall_timeout_ms")]
    pub overall_timeout_ms: u64,
}

fn default_gateway_url() -> String {
    "http://localhost:18789".to_string()
}
fn default_voice_id() -> String {
    "es_AR-daniela-high".to_string()
}
fn default_true() -> bool {
    true
}
fn default_session_key() -> String {
    "agent:main:main".to_string()
}
fn default_last_tab() -> String {
    "config".to_string()
}
fn default_silence_timeout_ms() -> u64 {
    // FASE 3 fix: default 3000 ms. Antes era 1500 ms pero eso cortaba
    // respuestas de LLMs cloud (gemma4:31b-cloud) que tienen pausas
    // entre chunks de streaming de 2-3 segundos. Con 3000 ms las
    // respuestas llegan completas.
    3000
}
fn default_overall_timeout_ms() -> u64 {
    // FASE 3 fix: default 120 s. Antes era 30 s pero las respuestas
    // largas del LLM cloud pueden tardar más de 30 s en completarse.
    120_000
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            gateway_url: default_gateway_url(),
            gateway_token: String::new(),
            voice_id: default_voice_id(),
            auto_speak: true,
            session_key: default_session_key(),
            last_tab: default_last_tab(),
            stt_model_id: String::new(),
            auto_send_after_dictation: false,
            silence_timeout_ms: default_silence_timeout_ms(),
            overall_timeout_ms: default_overall_timeout_ms(),
        }
    }
}

/// Ruta al archivo `settings.json` en el directorio de config del
/// proyecto. Mismo directorio que `device.key`.
fn settings_path() -> Option<PathBuf> {
    let base = directories::ProjectDirs::from("ai", "openclaw", "synapse-cortana")?;
    Some(base.config_dir().join("settings.json"))
}

/// Lee settings del disco. Devuelve `AppSettings::default()` si el
/// archivo no existe o está corrupto (loggeando el motivo).
pub fn load_settings() -> AppSettings {
    let Some(path) = settings_path() else {
        return AppSettings::default();
    };
    match std::fs::read_to_string(&path) {
        Ok(s) => match serde_json::from_str::<AppSettings>(&s) {
            Ok(set) => {
                info!(
                    "[SynapseCortana] settings cargados desde {}",
                    path.display()
                );
                set
            }
            Err(e) => {
                info!(
                    "[SynapseCortana] settings corruptos ({}), usando defaults: {e}",
                    path.display()
                );
                AppSettings::default()
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => AppSettings::default(),
        Err(e) => {
            info!(
                "[SynapseCortana] no pude leer {}: {e}, usando defaults",
                path.display()
            );
            AppSettings::default()
        }
    }
}

/// Escribe settings a disco con permisos `0600` (Unix). Devuelve
/// error legible si no se puede escribir.
pub fn save_settings(settings: &AppSettings) -> Result<(), String> {
    let path = settings_path().ok_or_else(|| "no se pudo resolver config_dir".to_string())?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("crear {}: {e}", parent.display()))?;
    }
    let json =
        serde_json::to_string_pretty(settings).map_err(|e| format!("serializar settings: {e}"))?;
    std::fs::write(&path, &json).map_err(|e| format!("escribir {}: {e}", path.display()))?;
    // Permisos 0600 en Unix (owner-only) porque el archivo contiene el token.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        let _ = std::fs::set_permissions(&path, perms);
    }
    Ok(())
}

/// Borra el archivo de settings y devuelve los defaults.
pub fn reset_settings_on_disk() -> AppSettings {
    if let Some(path) = settings_path() {
        let _ = std::fs::remove_file(&path);
    }
    AppSettings::default()
}

// ============================================
// TIPOS DE STREAM Y APP STATE
// ============================================

type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;
type WsSink = SplitSink<WsStream, Message>;

#[derive(Clone)]
pub struct AppState {
    /// URL base HTTP del gateway (ej. http://localhost:18789).
    pub gateway_url: Arc<Mutex<String>>,
    /// Token de autenticación compartido (opcional según la configuración
    /// del gateway).
    pub gateway_token: Arc<Mutex<String>>,
    /// `sessionKey` que se usará en los `chat.send`. Editable desde la UI
    /// (FASE 2.4.B); persiste entre arranques.
    pub session_key: Arc<Mutex<String>>,
    /// Indica si la última operación de `connect_to_gateway` tuvo éxito.
    pub connected: Arc<Mutex<bool>>,
    /// Mitad de escritura del stream WebSocket vivo. La mitad de
    /// lectura la consume la tarea de fondo.
    pub sink: Arc<AsyncMutex<Option<WsSink>>>,
    /// Buffer de eventos recibidos del gateway (chat, agent, etc.).
    /// El frontend los drena con `poll_gateway_events`.
    pub inbox: Arc<Mutex<Vec<GatewayEvent>>>,
    /// Contador incremental para asignar `id` a los `req`.
    pub req_counter: Arc<Mutex<u64>>,
    /// Identidad Ed25519 persistente del cliente. Se genera en el
    /// primer arranque y se guarda en disco para sobrevivir
    /// reinicios (así el `device.id` es estable y el pairing no se
    /// invalida cada vez).
    pub device: Arc<DeviceIdentity>,
    /// Motor TTS local (FASE 2). Carga perecosa: la primera llamada a
    /// `speak()` descarga y carga el modelo por defecto. Es seguro
    /// clonarlo porque todos los campos son `Arc<...>`.
    pub tts: Arc<TtsEngine>,
    /// Motor STT local (FASE 2.4.C). Mismo patrón: carga perezosa
    /// al invocar `stt_start`.
    pub stt: Arc<SttEngine>,
    /// Settings actuales (cacheados en memoria para acceso rápido
    /// desde los handlers Tauri sin re-leer el disco).
    pub settings: Arc<Mutex<AppSettings>>,
}

impl Default for AppState {
    fn default() -> Self {
        let device = DeviceIdentity::load_or_create().unwrap_or_else(|e| {
            info!("[SynapseCortana] no pude cargar la identidad del dispositivo: {e}");
            // Si falla, generamos una en memoria (no persistirá).
            DeviceIdentity::generate_in_memory()
        });
        // Cargar settings persistentes y usarlos para inicializar los
        // campos del estado (URL, token, session_key, etc.).
        let settings = load_settings();
        // FASE 2.5 (pre-carga TTS): la voz por defecto se descarga
        // silenciosamente en background al iniciar la app. Para cuando
        // el usuario envíe el primer mensaje, el modelo ya está listo
        // y la primera reproducción NO tarda ~1 min en descargar.
        //
        // IMPORTANTE: `Default::default()` se ejecuta ANTES del runtime
        // de Tokio (durante `tauri::Builder::manage`), por lo que NO
        // podemos usar `tokio::spawn` aquí. Usamos `std::thread::spawn`
        // y creamos nuestro propio runtime Tokio dentro del hilo.
        let tts_engine = Arc::new(TtsEngine::new());
        {
            let settings_voice = settings.voice_id.clone();
            let tts_clone = tts_engine.clone();
            std::thread::Builder::new()
                .name("tts-preload".into())
                .spawn(move || {
                    // Solo precargamos si la voz configurada existe en el
                    // catálogo. Si el usuario tenía una voz obsoleta
                    // persistida, usamos la default.
                    let voice_id = if tts::voice_by_id(&settings_voice).is_some() {
                        settings_voice
                    } else {
                        tts::DEFAULT_VOICE_ID.to_string()
                    };
                    info!("[tts] pre-cargando voz '{}' en background", voice_id);
                    // Crear runtime Tokio local para esta pre-carga.
                    let rt = match tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                    {
                        Ok(rt) => rt,
                        Err(e) => {
                            warn!("[tts] no pude crear runtime para pre-carga: {e}");
                            return;
                        }
                    };
                    rt.block_on(async move {
                        match tts_clone.set_voice(&voice_id).await {
                            Ok(_) => info!("[tts] pre-carga de voz '{}' completada", voice_id),
                            Err(e) => warn!(
                                "[tts] pre-carga de voz '{}' falló (se reintentará al primer uso): {}",
                                voice_id, e
                            ),
                        }
                    });
                })
                .expect("no pude crear hilo de pre-carga TTS");
        }
        Self {
            gateway_url: Arc::new(Mutex::new(settings.gateway_url.clone())),
            gateway_token: Arc::new(Mutex::new(settings.gateway_token.clone())),
            session_key: Arc::new(Mutex::new(settings.session_key.clone())),
            connected: Arc::new(Mutex::new(false)),
            sink: Arc::new(AsyncMutex::new(None)),
            inbox: Arc::new(Mutex::new(Vec::new())),
            req_counter: Arc::new(Mutex::new(0)),
            device: Arc::new(device),
            tts: tts_engine,
            stt: Arc::new(SttEngine::new()),
            settings: Arc::new(Mutex::new(settings)),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GatewayEvent {
    pub event: String,
    pub payload: serde_json::Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HelloOkInfo {
    pub protocol: Option<u64>,
    pub server_version: Option<String>,
    pub conn_id: Option<String>,
    pub features_methods: Option<Vec<String>>,
}

// ============================================
// DEVICE IDENTITY (Ed25519)
// ============================================
//
// Implementa la firma del `connect.challenge` nonce exigida por
// OpenClaw v4. La identidad es persistente: la primera vez se genera
// y se guarda en `~/.config/synapse-cortana/device.key`. En arranques
// posteriores se carga del disco, de modo que `device.id` (que es el
// SHA-256 de la clave pública) sea estable.

pub struct DeviceIdentity {
    signing_key: SigningKey,
}

impl DeviceIdentity {
    /// Genera una identidad nueva sin persistirla.
    pub fn generate_in_memory() -> Self {
        let mut csprng = OsRng;
        let signing_key = SigningKey::generate(&mut csprng);
        Self { signing_key }
    }

    /// Ruta del fichero de clave persistente.
    fn key_path() -> Option<PathBuf> {
        let base = directories::ProjectDirs::from("ai", "openclaw", "synapse-cortana")?;
        Some(base.config_dir().join("device.key"))
    }

    /// Carga la identidad desde disco o, si no existe, la genera y la
    /// persiste.
    pub fn load_or_create() -> Result<Self, String> {
        let path = Self::key_path().ok_or_else(|| "no se pudo resolver config_dir".to_string())?;
        if path.exists() {
            let pem = std::fs::read_to_string(&path)
                .map_err(|e| format!("leyendo {}: {e}", path.display()))?;
            return Self::from_pkcs8_pem(&pem);
        }
        // Generar y persistir.
        let id = Self::generate_in_memory();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("creando {}: {e}", parent.display()))?;
        }
        let pem = id.to_pkcs8_pem()?;
        std::fs::write(&path, pem.as_bytes())
            .map_err(|e| format!("escribiendo {}: {e}", path.display()))?;
        // Permisos 0600 en Unix.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
        }
        Ok(id)
    }

    /// Exporta la clave privada a PKCS8 PEM.
    pub fn to_pkcs8_pem(&self) -> Result<String, String> {
        use ed25519_dalek::pkcs8::EncodePrivateKey;
        use pkcs8::LineEnding;
        self.signing_key
            .to_pkcs8_pem(LineEnding::LF)
            .map(|s| s.to_string())
            .map_err(|e| format!("exportar PKCS8: {e}"))
    }

    /// Importa una clave privada desde PKCS8 PEM.
    pub fn from_pkcs8_pem(pem: &str) -> Result<Self, String> {
        use ed25519_dalek::pkcs8::DecodePrivateKey;
        let signing_key =
            SigningKey::from_pkcs8_pem(pem).map_err(|e| format!("importar PKCS8: {e}"))?;
        Ok(Self { signing_key })
    }

    /// Devuelve la clave pública en bytes crudos (32 bytes).
    pub fn public_key_bytes(&self) -> [u8; 32] {
        self.signing_key.verifying_key().to_bytes()
    }

    /// Codifica los 32 bytes raw de la clave pública como base64url.
    /// Es lo que OpenClaw espera en `connect.params.device.publicKey`.
    pub fn public_key_base64url(&self) -> String {
        let bytes = self.public_key_bytes();
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
    }

    /// `device.id` = SHA-256(publicKey_raw_32_bytes) en hex minúsculas.
    /// OpenClaw hace el fingerprint sobre los 32 bytes crudos de la
    /// clave pública, NO sobre el SPKI DER.
    pub fn device_id(&self) -> String {
        let raw = self.public_key_bytes();
        let mut hasher = Sha256::new();
        hasher.update(raw);
        let digest = hasher.finalize();
        hex::encode(digest)
    }

    /// Firma el payload **v2** que OpenClaw acepta. El payload es:
    ///   v2|deviceId|clientId|clientMode|role|scopes|signedAtMs|token|nonce
    /// (separador `|`). El servidor de OpenClaw primero prueba v3 y luego
    /// v2; como v3 está reservado para clientes con spec exacta, usamos
    /// v2 que ya validamos contra el gateway.
    ///
    /// Devuelve `(payload, signature_base64url)`.
    pub fn sign_v2(
        &self,
        client_id: &str,
        client_mode: &str,
        role: &str,
        scopes: &[String],
        token: &str,
        nonce: &str,
        signed_at_ms: i64,
    ) -> (String, String) {
        let scopes_csv = scopes.join(",");
        let payload = format!(
            "v2|{deviceId}|{clientId}|{clientMode}|{role}|{scopes}|{signedAtMs}|{token}|{nonce}",
            deviceId = self.device_id(),
            clientId = client_id,
            clientMode = client_mode,
            role = role,
            scopes = scopes_csv,
            signedAtMs = signed_at_ms,
            token = token,
            nonce = nonce,
        );
        let sig: Signature = self.signing_key.sign(payload.as_bytes());
        let sig_b64url = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(sig.to_bytes());
        (payload, sig_b64url)
    }
}

// ============================================
// MODELOS DE TRAMAS (OpenClaw WS Protocol v4)
// ============================================

#[derive(Serialize)]
struct ConnectRequest {
    #[serde(rename = "type")]
    msg_type: &'static str,
    id: String,
    method: &'static str,
    params: serde_json::Value,
}

#[derive(Serialize)]
struct RpcRequest {
    #[serde(rename = "type")]
    msg_type: &'static str,
    id: String,
    method: String,
    params: serde_json::Value,
}

/// Representa una trama entrante del gateway de OpenClaw.
///
/// Usamos discriminadores explícitos por el campo `"type"` en lugar de
/// `#[serde(untagged)]` porque el JSON del gateway tiene tanto `event`
/// como `res` y el orden de las variantes en untagged no es estable:
/// antes, el `connect.challenge` (un `event` legítimo) se deserializaba
/// como `Response { id: None, ok: None, error: None }` por accidente,
/// perdiendo el nonce y haciendo que el gateway cerrara la conexión.
#[derive(Deserialize, Debug)]
#[serde(tag = "type", rename_all = "lowercase")]
enum IncomingFrame {
    /// Trama de evento del gateway (`event: "..."`).
    Event {
        event: String,
        #[serde(default)]
        payload: Option<serde_json::Value>,
    },
    /// Trama de respuesta a una `req` nuestra (`res: ...`).
    #[serde(rename = "res")]
    Response {
        id: Option<String>,
        ok: Option<bool>,
        #[serde(default)]
        payload: Option<serde_json::Value>,
        #[serde(default)]
        error: Option<serde_json::Value>,
    },
}

// ============================================
// HELPERS
// ============================================

fn build_ws_url(http_url: &str) -> String {
    // OpenClaw monta el WebSocketServer en la raíz del HTTP server,
    // no en `/ws`. El cliente de referencia (OpenClaw/client.ts) usa
    // la URL HTTP tal cual para abrir el WebSocket.
    let mut url = http_url.to_string();
    if url.starts_with("https://") {
        url = url.replacen("https://", "wss://", 1);
    } else if url.starts_with("http://") {
        url = url.replacen("http://", "ws://", 1);
    }
    url.trim_end_matches('/').to_string()
}

fn next_req_id(state: &AppState) -> String {
    let mut counter = state.req_counter.lock().expect("req_counter poisoned");
    *counter += 1;
    format!("req-{}", *counter)
}

fn push_event(state: &AppState, event: &str, payload: serde_json::Value) {
    let mut inbox = state.inbox.lock().expect("inbox poisoned");
    if inbox.len() >= 256 {
        inbox.remove(0);
    }
    inbox.push(GatewayEvent {
        event: event.to_string(),
        payload,
    });
}

// ============================================
// COMANDOS TAURI — Configuración
// ============================================

#[tauri::command]
fn get_gateway_url(state: State<'_, AppState>) -> String {
    state
        .gateway_url
        .lock()
        .expect("gateway_url poisoned")
        .clone()
}

#[tauri::command]
fn set_gateway_url(url: String, state: State<'_, AppState>) -> Result<(), String> {
    let trimmed = url.trim().to_string();
    if !(trimmed.starts_with("http://") || trimmed.starts_with("https://")) {
        return Err("La URL del gateway debe comenzar con http:// o https://".to_string());
    }
    *state.gateway_url.lock().map_err(|e| e.to_string())? = trimmed;
    Ok(())
}

#[tauri::command]
fn get_gateway_token(state: State<'_, AppState>) -> String {
    state
        .gateway_token
        .lock()
        .expect("gateway_token poisoned")
        .clone()
}

#[tauri::command]
fn set_gateway_token(token: String, state: State<'_, AppState>) -> Result<(), String> {
    *state.gateway_token.lock().map_err(|e| e.to_string())? = token;
    Ok(())
}

#[tauri::command]
fn is_connected(state: State<'_, AppState>) -> bool {
    *state.connected.lock().expect("connected poisoned")
}

// ============================================
// COMANDOS TAURI — Health y Connect
// ============================================

/// Verifica vía HTTP si el gateway responde. No usa WebSocket.
#[tauri::command]
async fn check_gateway_connection(state: State<'_, AppState>) -> Result<bool, String> {
    let gateway_url = state.gateway_url.lock().map_err(|e| e.to_string())?.clone();

    for path in ["/health", "/"] {
        let url = format!("{}{}", gateway_url.trim_end_matches('/'), path);
        match reqwest::get(&url).await {
            Ok(resp) if resp.status().is_success() => return Ok(true),
            Ok(_) => continue,
            Err(_) => continue,
        }
    }
    Ok(false)
}

/// Inicia el handshake WebSocket con OpenClaw, deja la conexión viva
/// en `AppState` y lanza la tarea que drena los eventos entrantes.
#[tauri::command]
async fn connect_to_gateway(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<HelloOkInfo, String> {
    // Clonamos el AppState completo para poder usar referencias 'static
    // dentro de la tarea spawn. Como AppState solo contiene `Arc<...>`,
    // es barato clonarlo.
    let app_state: AppState = state.inner().clone();

    let gateway_url = app_state
        .gateway_url
        .lock()
        .map_err(|e| e.to_string())?
        .clone();
    let token = app_state
        .gateway_token
        .lock()
        .map_err(|e| e.to_string())?
        .clone();

    // 1) Construir URL ws:// y conectar.
    let ws_url = build_ws_url(&gateway_url);
    let mut request = ws_url
        .clone()
        .into_client_request()
        .map_err(|e| format!("URL inválida ({ws_url}): {e}"))?;
    request.headers_mut().insert(
        "User-Agent",
        HeaderValue::from_static("synapse-cortana/0.1.0"),
    );
    // FASE 2.3: el modo `ui` exige un header `Origin` permitido por
    // `gateway.controlUi.allowedOrigins` (por defecto solo el host
    // del gateway). Usamos la URL HTTP del gateway como origin.
    request.headers_mut().insert(
        "Origin",
        HeaderValue::from_str(&gateway_url).map_err(|e| format!("Origin inválido: {e}"))?,
    );

    let (ws, _response) = connect_async(request)
        .await
        .map_err(|e| format!("no se pudo conectar a {ws_url}: {e}"))?;

    // 2) Dividir el stream en sink (escritura) y stream (lectura).
    let (mut sink, mut read) = ws.split();

    // 3) Esperar `connect.challenge`.
    let (nonce, _ts): (String, i64) = loop {
        let msg = read
            .next()
            .await
            .ok_or_else(|| "stream cerrado antes del challenge".to_string())?;
        let raw = match msg {
            Ok(Message::Text(t)) => t.to_string(),
            Ok(Message::Binary(b)) => {
                String::from_utf8(b.to_vec()).map_err(|e| format!("binario no utf8: {e}"))?
            }
            Ok(Message::Close(c)) => {
                return Err(format!("gateway cerró antes del challenge: {:?}", c));
            }
            Ok(Message::Ping(_)) | Ok(Message::Pong(_)) => continue,
            Ok(_) => continue,
            Err(e) => return Err(format!("error leyendo challenge: {e}")),
        };

        let frame: IncomingFrame =
            serde_json::from_str(&raw).map_err(|e| format!("parsear challenge ({raw}): {e}"))?;

        match frame {
            IncomingFrame::Event {
                event,
                payload: Some(payload),
                ..
            } if event == "connect.challenge" => {
                let nonce = payload
                    .get("nonce")
                    .and_then(|v| v.as_str())
                    .ok_or("challenge sin nonce")?
                    .to_string();
                let ts = payload
                    .get("ts")
                    .and_then(|v| v.as_i64())
                    .ok_or("challenge sin ts")?;
                break (nonce, ts);
            }
            IncomingFrame::Event { event, payload, .. } => {
                // Cualquier otro evento que llegue antes del challenge se
                // ignora silenciosamente y se guarda en el inbox.
                if let Some(p) = payload {
                    push_event(&app_state, &event, p);
                }
            }
            IncomingFrame::Response {
                id: None, error, ..
            } => {
                // Una `res` sin `id` antes del challenge es inesperada.
                // No abortamos: la registramos y seguimos esperando.
                info!(
                    "[SynapseCortana] res inesperada antes del challenge: {}",
                    error
                        .as_ref()
                        .map(|e| e.to_string())
                        .unwrap_or_else(|| "null".to_string())
                );
            }
            IncomingFrame::Response { .. } => continue,
        }
    };

    // 4) Construir y firmar el bloque `device` (Ed25519 v2).
    let device_id = app_state.device.device_id();
    let public_key_b64url = app_state.device.public_key_base64url();
    // OpenClaw normaliza el platform con `process.platform` y lo
    // canonicaliza a minúsculas. El handshake rechaza IDs/modes de
    // cliente desconocidos, así que usamos los enums oficiales
    // (ver packages/gateway-protocol/src/client-info.ts).
    let platform = match std::env::consts::OS {
        "macos" => "darwin",
        "windows" => "win32",
        other => other,
    };
    // FASE 2.3: usamos `client.id = "webchat-ui"` + `client.mode = "ui"`
    // en lugar de `gateway-client` / `backend`. Esto es lo que hace que
    // el gateway nos entregue los eventos del LLM (`chat.delta`,
    // `agent`, etc.) — en modo `backend` el gateway acepta el chat
    // pero no emite eventos. El handshake sigue funcionando con
    // scopes `operator.read`/`operator.write` porque el gateway
    // reconoce `webchat-ui` como cliente de chat en loopback.
    let client_id = "webchat-ui";
    let client_mode = "ui";
    let role = "operator";
    let scopes: Vec<String> = vec!["operator.read".to_string(), "operator.write".to_string()];
    let signed_at_ms: i64 = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| format!("reloj: {e}"))?
        .as_millis() as i64;

    let (_payload, signature_b64url) = app_state.device.sign_v2(
        client_id,
        client_mode,
        role,
        &scopes,
        &token,
        &nonce,
        signed_at_ms,
    );

    let device_block = serde_json::json!({
        "id": device_id,
        "publicKey": public_key_b64url,
        "signature": signature_b64url,
        "signedAt": signed_at_ms,
        "nonce": nonce,
    });

    // 5) Enviar `connect` con el bloque `device`.
    let mut connect_params = serde_json::json!({
        "minProtocol": 3,
        "maxProtocol": 4,
        "client": {
            "id": client_id,
            "version": env!("CARGO_PKG_VERSION"),
            "platform": platform,
            "mode": client_mode
        },
        "role": role,
        "scopes": scopes,
        "caps": [],
        "commands": [],
        "permissions": {},
        "locale": "es-ES",
        "userAgent": format!("synapse-cortana/{}", env!("CARGO_PKG_VERSION")),
        "device": device_block
    });
    if !token.is_empty() {
        if let Some(obj) = connect_params.as_object_mut() {
            obj.insert("auth".to_string(), serde_json::json!({ "token": token }));
        }
    }

    let connect_id = next_req_id(&app_state);
    let connect_frame = ConnectRequest {
        msg_type: "req",
        id: connect_id.clone(),
        method: "connect",
        params: connect_params,
    };
    let connect_text =
        serde_json::to_string(&connect_frame).map_err(|e| format!("serializar connect: {e}"))?;
    sink.send(Message::Text(connect_text))
        .await
        .map_err(|e| format!("enviar connect: {e}"))?;

    // 5) Esperar `hello-ok` o un error de device-auth.
    let hello_ok = loop {
        let msg = read
            .next()
            .await
            .ok_or_else(|| "stream cerrado esperando hello-ok".to_string())?;
        let raw = match msg {
            Ok(Message::Text(t)) => t.to_string(),
            Ok(Message::Binary(b)) => {
                String::from_utf8(b.to_vec()).map_err(|e| format!("binario no utf8: {e}"))?
            }
            Ok(Message::Close(c)) => {
                return Err(format!("gateway cerró antes de hello-ok: {:?}", c));
            }
            Ok(Message::Ping(_)) | Ok(Message::Pong(_)) => continue,
            Ok(_) => continue,
            Err(e) => return Err(format!("error leyendo hello-ok: {e}")),
        };

        let frame: IncomingFrame =
            serde_json::from_str(&raw).map_err(|e| format!("parsear hello-ok ({raw}): {e}"))?;

        match frame {
            IncomingFrame::Response {
                id: Some(resp_id),
                ok: Some(true),
                payload: Some(payload),
                ..
            } if resp_id == connect_id => {
                if payload.get("type").and_then(|v| v.as_str()) == Some("hello-ok") {
                    break payload;
                }
                return Err(format!(
                    "respuesta inesperada al connect: {}",
                    serde_json::to_string(&payload).unwrap_or_default()
                ));
            }
            IncomingFrame::Response {
                id: Some(resp_id),
                ok,
                error,
                ..
            } if resp_id == connect_id => {
                let err_str = error
                    .as_ref()
                    .map(|v| serde_json::to_string(v).unwrap_or_default())
                    .unwrap_or_default();
                let details_code = error
                    .as_ref()
                    .and_then(|v| v.get("details"))
                    .and_then(|d| d.get("code"))
                    .and_then(|c| c.as_str())
                    .unwrap_or("");
                if details_code.starts_with("DEVICE_AUTH_") {
                    return Err(format!(
                        "El gateway requiere autenticación de dispositivo ({}). \
                         En esta FASE 1 SynapseCortana solo soporta el modo loopback/backend \
                         o auth=none. Configura el gateway con `gateway.auth.mode = \"none\"` \
                         (solo para ingreso privado) o habilita la ruta loopback backend.",
                        details_code
                    ));
                }
                return Err(format!(
                    "connect rechazado (ok={:?}): {}",
                    ok.unwrap_or(false),
                    err_str
                ));
            }
            IncomingFrame::Response { .. } => continue,
            IncomingFrame::Event { event, payload, .. } => {
                if let Some(p) = payload {
                    push_event(&app_state, &event, p);
                }
            }
        }
    };

    // 6) Conexión aceptada. Guardamos el sink y lanzamos la tarea
    // que consume el stream de lectura.
    let info = HelloOkInfo {
        protocol: hello_ok.get("protocol").and_then(|v| v.as_u64()),
        server_version: hello_ok
            .get("server")
            .and_then(|s| s.get("version"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        conn_id: hello_ok
            .get("server")
            .and_then(|s| s.get("connId"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        features_methods: hello_ok
            .get("features")
            .and_then(|f| f.get("methods"))
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|x| x.as_str().map(|s| s.to_string()))
                    .collect()
            }),
    };

    {
        let mut guard = app_state.sink.lock().await;
        *guard = Some(sink);
    }
    *app_state.connected.lock().map_err(|e| e.to_string())? = true;

    // 7) Tarea en background: lee del stream y llena el inbox
    // + emite cada evento al frontend. Como `app_state` es `Clone`
    // y solo contiene `Arc<...>`, podemos moverlo al closure.
    let app_state_for_pump = app_state.clone();
    tokio::spawn(async move {
        run_event_pump(read, app_state_for_pump, app).await;
    });

    println!(
        "[SynapseCortana] hello-ok recibido. protocol={:?}, connId={:?}",
        info.protocol, info.conn_id
    );
    Ok(info)
}

async fn run_event_pump(
    mut read: futures_util::stream::SplitStream<WsStream>,
    state: AppState,
    app: AppHandle,
) {
    while let Some(msg) = read.next().await {
        let raw = match msg {
            Ok(Message::Text(t)) => t.to_string(),
            Ok(Message::Binary(b)) => match String::from_utf8(b.to_vec()) {
                Ok(s) => s,
                Err(_) => continue,
            },
            Ok(Message::Close(_)) => break,
            Ok(Message::Ping(_)) | Ok(Message::Pong(_)) => continue,
            Ok(_) => continue,
            Err(_) => break,
        };

        let frame: Result<IncomingFrame, _> = serde_json::from_str(&raw);
        let parsed = match frame {
            Ok(f) => f,
            Err(_) => continue,
        };

        match parsed {
            IncomingFrame::Event { event, payload, .. } => {
                let payload = payload.unwrap_or(serde_json::Value::Null);
                push_event(&state, &event, payload.clone());
                let _ = app.emit("gateway:event", GatewayEvent { event, payload });
            }
            IncomingFrame::Response {
                id,
                ok,
                payload,
                error,
                ..
            } => {
                // FASE 2.4.B: las respuestas a RPCs (sessions.list,
                // sessions.resolve, etc.) se guardan en el inbox como
                // un GatewayEvent con event="res" y el payload completo
                // (incluyendo `id`, `ok`, `payload`, `error`) para que
                // `send_request_and_wait` las pueda encontrar.
                let mut res_payload = serde_json::Map::new();
                if let Some(id) = id {
                    res_payload.insert("id".to_string(), serde_json::Value::String(id));
                }
                if let Some(ok) = ok {
                    res_payload.insert("ok".to_string(), serde_json::Value::Bool(ok));
                }
                if let Some(p) = payload {
                    res_payload.insert("payload".to_string(), p);
                }
                if let Some(e) = error {
                    res_payload.insert("error".to_string(), e);
                }
                let res_value = serde_json::Value::Object(res_payload);
                push_event(&state, "res", res_value.clone());
                let _ = app.emit(
                    "gateway:event",
                    GatewayEvent {
                        event: "res".to_string(),
                        payload: res_value,
                    },
                );
            }
        }
    }

    *state.connected.lock().expect("connected poisoned") = false;
    let _ = app.emit("gateway:disconnected", serde_json::Value::Null);
}

// ============================================
// COMANDOS TAURI — RPC sobre la conexión viva
// ============================================

/// Envía un mensaje al canal por defecto vía `chat.send`.
#[tauri::command]
async fn send_message_to_gateway(
    message: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    if message.trim().is_empty() {
        return Err("El mensaje no puede estar vacío".to_string());
    }
    if !*state.connected.lock().map_err(|e| e.to_string())? {
        return Err("No hay conexión activa con el gateway".to_string());
    }

    // Para esta FASE 1 no esperamos la respuesta de chat.send en línea;
    // el frontend verá los eventos de chat/agent a través del inbox
    // que actualiza el event pump en background.
    let mut guard = state.sink.lock().await;
    let sink = guard
        .as_mut()
        .ok_or_else(|| "sink no disponible a pesar de connected=true".to_string())?;

    let id = next_req_id(&state);
    // FASE 2.3: el gateway v4 cambió la API de chat.send. Ahora
    // requiere `message` (no `text`), `sessionKey` (no `channel`) y
    // `idempotencyKey` único por envío.
    // FASE 2.4.B: el sessionKey se lee de AppState (que se hidrata
    // desde settings). El usuario puede cambiarlo desde la UI.
    let session_key = state
        .session_key
        .lock()
        .map_err(|e| format!("session_key poisoned: {e}"))?
        .clone();
    let idempotency_key = format!(
        "synapse-cortana-{}-{}",
        id,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| format!("reloj: {e}"))?
            .as_nanos()
    );
    let frame = RpcRequest {
        msg_type: "req",
        id: id.clone(),
        method: "chat.send".to_string(),
        params: serde_json::json!({
            "message": message,
            "sessionKey": session_key,
            "idempotencyKey": idempotency_key
        }),
    };
    let text = serde_json::to_string(&frame).map_err(|e| format!("serializar chat.send: {e}"))?;
    sink.send(Message::Text(text))
        .await
        .map_err(|e| format!("enviar chat.send: {e}"))?;
    Ok(id)
}

/// Drena los eventos almacenados en el inbox. Lo usa el frontend
/// para mostrar respuestas que llegaron mientras la UI no estaba activa.
#[tauri::command]
fn poll_gateway_events(state: State<'_, AppState>) -> Vec<GatewayEvent> {
    let mut inbox = state.inbox.lock().expect("inbox poisoned");
    std::mem::take(&mut *inbox)
}

/// Cierra la conexión WebSocket.
#[tauri::command]
async fn disconnect_from_gateway(state: State<'_, AppState>) -> Result<(), String> {
    let mut guard = state.sink.lock().await;
    if let Some(mut s) = guard.take() {
        let _ = s.send(Message::Close(None)).await;
        let _ = s.close().await;
    }
    *state.connected.lock().map_err(|e| e.to_string())? = false;
    Ok(())
}

// ============================================
// COMANDOS TAURI — chat + speak (FASE 2.3)
// ============================================

/// Resultado combinado: lo que respondió el agente + el audio TTS
/// sintetizado localmente, en una sola llamada RPC.
#[derive(Serialize)]
pub struct ChatAndSpeakResult {
    /// Texto completo que devolvió el agente (acumulado de eventos).
    pub agent_text: String,
    /// Audio WAV codificado en base64 (PCM 16-bit mono @ 22050 Hz).
    pub audio_base64: String,
    pub sample_rate: i32,
    pub num_samples: usize,
    pub duration_ms: u64,
    pub voice_id: String,
    /// `req_id` del `chat.send` original.
    pub req_id: String,
    /// `sessionKey` usado en el `chat.send`. El frontend lo necesita
    /// para ignorar los chunks de streaming que lleguen por la misma
    /// sesión mientras `chat_and_speak` ya está sintetizando el audio,
    /// evitando duplicidad de audio + texto.
    pub session_key: String,
    /// Milisegundos totales desde el envío del `chat.send` hasta
    /// la finalización de la inferencia TTS. Útil para mostrar
    /// "latencia end-to-end" en la UI.
    pub elapsed_ms: u64,
}

/// Envía un mensaje al gateway vía `chat.send`, espera la respuesta
/// del agente, la sintetiza con el TTS local, y devuelve texto+audio.
///
/// **Estrategia de espera**:
///   1. Envía `chat.send` con un `id` único (`req-...`).
///   2. Drena el `inbox` periódicamente (cada 100 ms) buscando
///      eventos cuyo `payload.text`/`message`/`delta`/`content`
///      aporte al texto de la respuesta. La heurística acepta
///      eventos `chat`, `agent`, `chat.message`, `agent.message`,
///      `chat.delta`, `session.message`, `session.message.delta`.
///   3. Considera la respuesta "completa" cuando pasa
///      `silence_timeout_ms` (por defecto 1500 ms) sin nuevos
///      eventos que aporten texto, **o** cuando llega un evento
///      `chat.done` / `agent.done` / `chat.abort` / `agent.abort`.
///   4. Si no llega nada en `overall_timeout_ms` (por defecto 60 s),
///      devuelve error.
///
/// Si el texto acumulado está vacío, no sintetiza audio y devuelve
/// `audio_base64 = ""`.
#[tauri::command]
async fn chat_and_speak(
    message: String,
    voice_id: Option<String>,
    silence_timeout_ms: Option<u64>,
    overall_timeout_ms: Option<u64>,
    state: State<'_, AppState>,
) -> Result<ChatAndSpeakResult, String> {
    use std::time::{Duration, Instant};

    if message.trim().is_empty() {
        return Err("El mensaje no puede estar vacío".to_string());
    }
    if !*state.connected.lock().map_err(|e| e.to_string())? {
        return Err("No hay conexión activa con el gateway".to_string());
    }

    let voice_ref = voice_id.as_deref();
    // FASE 2.5: leer defaults desde settings persistentes. Si el caller
    // (frontend) pasa un valor explícito, ese gana. Esto permite que el
    // usuario configure timeouts desde la UI sin recompilar.
    let default_silence_ms = state
        .settings
        .lock()
        .map(|s| s.silence_timeout_ms)
        .unwrap_or_else(|_| default_silence_timeout_ms());
    let default_overall_ms = state
        .settings
        .lock()
        .map(|s| s.overall_timeout_ms)
        .unwrap_or_else(|_| default_overall_timeout_ms());
    let silence_ms = silence_timeout_ms.unwrap_or(default_silence_ms);
    let overall_ms = overall_timeout_ms.unwrap_or(default_overall_ms);

    // FASE 2.4.B: sessionKey desde AppState (settings persistentes).
    let session_key = state
        .session_key
        .lock()
        .map_err(|e| format!("session_key poisoned: {e}"))?
        .clone();
    // FASE 2.4 (defensa): si sessionKey está vacío (por ejemplo,
    // después de un reset o settings corruptos), usar el default. NO
    // enviamos `chat.send` sin sessionKey porque eso crearía una sesión
    // nueva en el gateway y mezclaría historiales.
    let session_key = if session_key.trim().is_empty() {
        "agent:main:main".to_string()
    } else {
        session_key
    };
    info!(
        "[chat_and_speak] usando sessionKey='{}', mensaje='{}'",
        session_key,
        message.chars().take(80).collect::<String>()
    );

    // 1) Enviar chat.send y capturar req_id.
    let req_id = {
        let mut guard = state.sink.lock().await;
        let sink = guard
            .as_mut()
            .ok_or_else(|| "sink no disponible a pesar de connected=true".to_string())?;
        let id = next_req_id(&state);
        // FASE 2.3: usar la nueva API de chat.send (message/sessionKey/idempotencyKey).
        let idempotency_key = format!(
            "synapse-cortana-{}-{}",
            id,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|e| format!("reloj: {e}"))?
                .as_nanos()
        );
        let frame = RpcRequest {
            msg_type: "req",
            id: id.clone(),
            method: "chat.send".to_string(),
            params: serde_json::json!({
                "message": message,
                "sessionKey": session_key,
                "idempotencyKey": idempotency_key
            }),
        };
        let text =
            serde_json::to_string(&frame).map_err(|e| format!("serializar chat.send: {e}"))?;
        sink.send(Message::Text(text))
            .await
            .map_err(|e| format!("enviar chat.send: {e}"))?;
        id
    };

    // 2) Esperar eventos del inbox.
    let overall_deadline = Instant::now() + Duration::from_millis(overall_ms);
    let mut last_event_at = Instant::now();
    let mut accumulated_text = String::new();
    let mut done_event_seen = false;

    loop {
        // Drenar inbox.
        let drained: Vec<GatewayEvent> = {
            let mut inbox = state.inbox.lock().expect("inbox poisoned");
            std::mem::take(&mut *inbox)
        };
        let mut got_new_text = false;
        let mut got_terminal = false;
        for ev in drained {
            if !ev.event.is_empty() {
                // Eventos terminales: la respuesta terminó.
                if matches!(
                    ev.event.as_str(),
                    "chat.done" | "agent.done" | "chat.abort" | "agent.abort"
                ) {
                    got_terminal = true;
                    continue;
                }
                // Eventos de texto: extraer contenido.
                let chunk = extract_text_chunk(&ev.payload);
                if !chunk.is_empty() {
                    // NO insertar espacio entre chunks. El gateway ya envía
                    // los deltas con los espacios incluidos. Insertar un
                    // espacio extra rompe palabras ("Cl aro" en vez de "Claro")
                    // cuando el límite de un chunk cae en medio de una palabra.
                    accumulated_text.push_str(&chunk);
                    got_new_text = true;
                }
            }
        }
        if got_new_text {
            last_event_at = Instant::now();
        }
        if got_terminal {
            done_event_seen = true;
            break;
        }
        // Salir si pasó el silencio.
        if last_event_at.elapsed() >= Duration::from_millis(silence_ms)
            && !accumulated_text.is_empty()
        {
            break;
        }
        // Salir si pasó el timeout global.
        if Instant::now() >= overall_deadline {
            if accumulated_text.is_empty() {
                return Err(format!(
                    "timeout ({overall_ms} ms) esperando respuesta del agente"
                ));
            }
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    let agent_text = accumulated_text.trim().to_string();
    if agent_text.is_empty() && !done_event_seen {
        return Err("el agente no devolvió texto".to_string());
    }

    // 3) Sintetizar con TTS local (si hay texto).
    let started_total = Instant::now();
    let (audio_base64, sample_rate, num_samples, duration_ms, used_voice_id) =
        if agent_text.is_empty() {
            (String::new(), 0, 0, 0, String::new())
        } else {
            let (samples, sr) = state.tts.synthesize(&agent_text, voice_ref).await?;
            let wav_bytes = tts::samples_f32_to_wav_bytes(&samples, sr);
            let b64 = base64::engine::general_purpose::STANDARD.encode(&wav_bytes);
            let n = samples.len();
            let dur_ms = (n as u64 * 1000) / (sr as u64).max(1);
            let status = state.tts.status().await;
            (b64, sr, n, dur_ms, status.voice_id.unwrap_or_default())
        };

    Ok(ChatAndSpeakResult {
        agent_text,
        audio_base64,
        sample_rate,
        num_samples,
        duration_ms,
        voice_id: used_voice_id,
        req_id,
        session_key,
        elapsed_ms: started_total.elapsed().as_millis() as u64,
    })
}

/// Extrae texto de un payload de evento del gateway. Soporta los
/// campos más comunes según el esquema v4 de OpenClaw:
///   - `deltaText`: chunks streaming de la respuesta del LLM (eventos `chat`).
///   - `message`: mensaje final del turno (evento terminal `chat`).
///   - `text`, `content`: legacy / otros canales.
/// Si no encuentra ninguno, devuelve cadena vacía.
fn extract_text_chunk(payload: &serde_json::Value) -> String {
    for key in ["deltaText", "message", "text", "content", "delta"] {
        if let Some(s) = payload.get(key).and_then(|v| v.as_str()) {
            return s.to_string();
        }
    }
    String::new()
}

// ============================================
// CLI TEST HANDSHAKE (modo sin GUI para SSH/headless)
// ============================================

/// Ejecuta el handshake WebSocket contra el gateway de OpenClaw e
/// imprime el resultado en stdout. Diseñado para entornos sin display
/// gráfico donde la GUI de Tauri no puede arrancar. Devuelve el código
/// de salida del proceso (0 = hello-ok, 1 = error).
pub fn cli_test_handshake(gateway_url: &str, token: Option<&str>) -> i32 {
    use futures_util::{SinkExt, StreamExt};
    use std::time::{SystemTime, UNIX_EPOCH};
    use tokio_tungstenite::{
        connect_async,
        tungstenite::{client::IntoClientRequest, http::HeaderValue, Message},
    };

    info!("[cli-handshake] Iniciando contra {gateway_url}");

    // Resolver token: argumento > env OPENCLAW_TOKEN > puerta gateway_url vacía
    let resolved_token = token
        .map(|s| s.to_string())
        .or_else(|| std::env::var("OPENCLAW_TOKEN").ok())
        .unwrap_or_default();

    // Cargar o crear la identidad Ed25519 persistente
    let device = match crate::DeviceIdentity::load_or_create() {
        Ok(d) => d,
        Err(e) => {
            error!("[cli-handshake] ERROR creando identidad: {e}");
            return 1;
        }
    };
    let device_id = device.device_id();
    let public_key_b64url = device.public_key_base64url();
    info!("[cli-handshake] device.id = {device_id}");
    info!("[cli-handshake] publicKey = {public_key_b64url}");

    // Construir URL ws://
    let mut ws_url = gateway_url.to_string();
    if ws_url.starts_with("https://") {
        ws_url = ws_url.replacen("https://", "wss://", 1);
    } else if ws_url.starts_with("http://") {
        ws_url = ws_url.replacen("http://", "ws://", 1);
    }
    ws_url = ws_url.trim_end_matches('/').to_string();

    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(r) => r,
        Err(e) => {
            error!("[cli-handshake] ERROR creando runtime: {e}");
            return 1;
        }
    };

    let result: Result<(), String> = rt.block_on(async {
        let ws_url_for_err = ws_url.clone();
        let mut request = ws_url
            .into_client_request()
            .map_err(|e| format!("URL inválida ({ws_url_for_err}): {e}"))?;
        request.headers_mut().insert(
            "User-Agent",
            HeaderValue::from_static("synapse-cortana/0.1.0"),
        );
        let (mut ws, _response) = connect_async(request)
            .await
            .map_err(|e| format!("no se pudo conectar a {ws_url_for_err}: {e}"))?;
        info!("[cli-handshake] WS conectado a {ws_url_for_err}");

        // Esperar connect.challenge
        let nonce = loop {
            // `ws.next().await` devuelve `Option<Result<Message, Error>>`.
            // Hay que desenvolver primero la `Option` y luego el `Result`.
            let msg = match ws.next().await {
                Some(Ok(m)) => m,
                Some(Err(e)) => return Err(format!("ws error antes del challenge: {e}")),
                None => return Err("stream cerrado antes del challenge".to_string()),
            };
            let raw = match msg {
                Message::Text(t) => t.to_string(),
                Message::Binary(b) => {
                    String::from_utf8(b.to_vec()).map_err(|e| format!("binario no utf8: {e}"))?
                }
                Message::Ping(_) | Message::Pong(_) | Message::Frame(_) => continue,
                Message::Close(c) => {
                    return Err(format!("gateway cerró antes del challenge: {c:?}"))
                }
            };
            let v: serde_json::Value =
                serde_json::from_str(&raw).map_err(|e| format!("parse challenge: {e}"))?;
            if v["event"] == "connect.challenge" {
                break v["payload"]["nonce"]
                    .as_str()
                    .ok_or("challenge sin nonce")?
                    .to_string();
            }
        };
        info!("[cli-handshake] challenge recibido, nonce = {nonce}");

        // Construir y firmar payload v2
        let platform = match std::env::consts::OS {
            "macos" => "darwin",
            "windows" => "win32",
            other => other,
        };
        let client_id = "gateway-client";
        let client_mode = "backend";
        let role = "operator";
        let scopes_vec = vec!["operator.read".to_string(), "operator.write".to_string()];
        let signed_at_ms: i64 = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| format!("reloj: {e}"))?
            .as_millis() as i64;
        let (_payload, signature_b64url) = device.sign_v2(
            client_id,
            client_mode,
            role,
            &scopes_vec,
            &resolved_token,
            &nonce,
            signed_at_ms,
        );

        // Construir el connect
        let mut connect_params = serde_json::json!({
            "minProtocol": 3,
            "maxProtocol": 4,
            "client": {
                "id": client_id,
                "version": env!("CARGO_PKG_VERSION"),
                "platform": platform,
                "mode": client_mode,
            },
            "role": role,
            "scopes": scopes_vec,
            "caps": [],
            "commands": [],
            "permissions": {},
            "locale": "es-ES",
            "userAgent": format!("synapse-cortana/{}", env!("CARGO_PKG_VERSION")),
            "device": {
                "id": device_id,
                "publicKey": public_key_b64url,
                "signature": signature_b64url,
                "signedAt": signed_at_ms,
                "nonce": nonce,
            }
        });
        if !resolved_token.is_empty() {
            connect_params["auth"] = serde_json::json!({ "token": resolved_token });
        }

        let connect = serde_json::json!({
            "type": "req",
            "id": "cli-test-handshake",
            "method": "connect",
            "params": connect_params,
        });
        let text = serde_json::to_string(&connect).map_err(|e| format!("serializar: {e}"))?;
        ws.send(Message::Text(text.into()))
            .await
            .map_err(|e| format!("enviar connect: {e}"))?;
        info!("[cli-handshake] connect enviado");

        // Esperar respuesta
        let resp = loop {
            // `tokio::time::timeout(_, ws.next())` también produce
            // `Result<Option<Result<Message, Error>>, Elapsed>`. Hay que
            // desenvolver las tres capas (timeout -> Option -> Result).
            let recv =
                match tokio::time::timeout(std::time::Duration::from_secs(10), ws.next()).await {
                    Ok(Some(Ok(m))) => m,
                    Ok(Some(Err(e))) => return Err(format!("ws error esperando respuesta: {e}")),
                    Ok(None) => return Err("stream cerrado esperando respuesta".to_string()),
                    Err(_) => return Err("timeout esperando respuesta".to_string()),
                };
            let raw = match recv {
                Message::Text(t) => t.to_string(),
                Message::Binary(b) => String::from_utf8(b.to_vec()).unwrap_or_default(),
                Message::Ping(_) | Message::Pong(_) | Message::Frame(_) => continue,
                Message::Close(c) => {
                    return Err(format!("gateway cerró esperando respuesta: {c:?}"))
                }
            };
            let v: serde_json::Value =
                serde_json::from_str(&raw).map_err(|e| format!("parse: {e}"))?;
            if v["type"] == "res" {
                break v;
            }
        };

        let ok = resp["ok"] == serde_json::Value::Bool(true);
        if ok {
            let hello = &resp["payload"];
            info!("[cli-handshake] ✅ HANDSHAKE OK");
            info!("[cli-handshake] protocol = {}", hello["protocol"]);
            info!(
                "[cli-handshake] server.version = {}",
                hello["server"]["version"]
            );
            info!("[cli-handshake] connId = {}", hello["server"]["connId"]);
            info!(
                "[cli-handshake] auth.role = {}",
                resp.get("payload")
                    .and_then(|p| p.get("auth"))
                    .and_then(|a| a.get("role"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("?")
            );
            info!(
                "[cli-handshake] auth.scopes = {}",
                serde_json::to_string(
                    resp.get("payload")
                        .and_then(|p| p.get("auth"))
                        .and_then(|a| a.get("scopes"))
                        .unwrap_or(&serde_json::Value::Null)
                )
                .unwrap_or_default()
            );
            // Imprimir toda la respuesta como JSON pretty
            println!(
                "{}",
                serde_json::to_string_pretty(&resp).unwrap_or_default()
            );
            Ok(())
        } else {
            error!("[cli-handshake] ❌ RECHAZADO");
            info!(
                "[cli-handshake] code = {}",
                resp.get("error")
                    .and_then(|e| e.get("code"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("?")
            );
            info!(
                "[cli-handshake] message = {}",
                resp.get("error")
                    .and_then(|e| e.get("message"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("?")
            );
            if let Some(details) = resp.get("error").and_then(|e| e.get("details")) {
                info!("[cli-handshake] details = {}", details);
            }
            Err("handshake rechazado".to_string())
        }
    });

    match result {
        Ok(()) => 0,
        Err(e) => {
            error!("[cli-handshake] ERROR: {e}");
            1
        }
    }
}

// ============================================
// CLI TEST TTS (FASE 2: sin GUI, sin gateway)
// ============================================

/// Ejecuta el TTS local con una frase fija y guarda el WAV resultante.
/// Pensado para validar el motor TTS sin necesidad de display ni de
/// que el gateway de OpenClaw esté corriendo.
///
/// Argumentos (todos opcionales):
///   `--voice <id>`  ID de voz del catálogo (por defecto la voz por defecto).
///   `--text  <txt>` Texto a sintetizar (por defecto una frase de prueba).
///   `--out   <path>` Ruta del WAV de salida (por defecto
///                    `/tmp/synapse-cortana-test.wav`).
pub fn cli_test_speak(voice: Option<&str>, text: Option<&str>, out: Option<&str>) -> i32 {
    let voice_id = voice.unwrap_or(tts::DEFAULT_VOICE_ID);
    let phrase =
        text.unwrap_or("Hola, soy Cortana. La fase dos del proyecto Synapse ya está hablando.");
    let out_path = out.unwrap_or("/tmp/synapse-cortana-test.wav").to_string();

    info!("[cli-speak] voz      = {voice_id}");
    info!("[cli-speak] texto    = {phrase}");
    info!("[cli-speak] salida   = {out_path}");

    if tts::voice_by_id(voice_id).is_none() {
        error!("[cli-speak] ERROR: voz desconocida '{voice_id}'");
        info!("[cli-speak] voces disponibles:");
        for v in tts::VOICE_CATALOG {
            info!(
                "[cli-speak]   - {} ({}, ~{} MB)",
                v.id, v.label, v.size_mb_approx
            );
        }
        return 1;
    }

    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(r) => r,
        Err(e) => {
            error!("[cli-speak] ERROR creando runtime: {e}");
            return 1;
        }
    };

    let engine = TtsEngine::new();
    let result: Result<(usize, i32, std::time::Duration), String> = rt.block_on(async {
        // Asegura que la voz está descargada/cargada.
        engine
            .set_voice(voice_id)
            .await
            .map_err(|e| format!("set_voice: {e}"))?;
        let started = std::time::Instant::now();
        let (n_samples, sample_rate) = engine
            .synthesize_to_wav(phrase, Some(voice_id), &out_path)
            .await?;
        let elapsed = started.elapsed();
        Ok((n_samples, sample_rate, elapsed))
    });

    match result {
        Ok((n, sr, elapsed)) => {
            let secs_audio = n as f32 / sr as f32;
            let rtf = elapsed.as_secs_f32() / secs_audio;
            info!("[cli-speak] ✅ OK");
            info!(
                "[cli-speak] samples   = {n} ({:.2} s de audio @ {} Hz)",
                secs_audio, sr
            );
            info!(
                "[cli-speak] latencia  = {:.2} s (RTF ≈ {:.2})",
                elapsed.as_secs_f32(),
                rtf
            );
            info!("[cli-speak] WAV guardado en {out_path}");
            0
        }
        Err(e) => {
            error!("[cli-speak] ❌ ERROR: {e}");
            1
        }
    }
}

// ============================================
// COMANDOS TAURI — TTS (FASE 2.2)
// ============================================

/// Devuelve el catálogo de voces disponibles para TTS.
/// El frontend lo usa para llenar el selector de voz.
#[tauri::command]
fn tts_list_voices() -> Vec<VoiceSpec> {
    tts::VOICE_CATALOG.to_vec()
}

/// Devuelve el estado actual del motor TTS (cargado o no, qué voz,
/// sample rate, último error). Útil para que el frontend muestre
/// un indicador "TTS listo" o "descargando voz...".
///
/// Tauri exige que los comandos async con `State` devuelvan `Result`,
/// así que envolvemos en `Ok`. Los errores en el motor se exponen
/// dentro de `TtsStatus.last_error` (no en este `Result`).
#[tauri::command]
async fn tts_status(state: State<'_, AppState>) -> Result<TtsStatus, String> {
    Ok(state.tts.status().await)
}

/// Cambia la voz activa del TTS. Si la voz no está en disco, la
/// descarga del tarball oficial de `k2-fsa` (puede tardar ~1 min la
/// primera vez). Devuelve el nuevo estado.
#[tauri::command]
async fn tts_set_voice(voice_id: String, state: State<'_, AppState>) -> Result<TtsStatus, String> {
    state.tts.set_voice(&voice_id).await?;
    Ok(state.tts.status().await)
}

/// Sintetiza el texto con el TTS local y devuelve el WAV en base64,
/// junto con metadatos (sample rate, duración, número de samples).
///
/// Si `voice_id` es `None` o vacía, usa la voz por defecto.
///
/// El frontend puede hacer `atob(base64)` y construir un Blob para
/// reproducir con `<audio>` o WebAudio API.
#[derive(Serialize)]
pub struct TtsSynthesizeResult {
    pub audio_base64: String,
    pub sample_rate: i32,
    pub num_samples: usize,
    pub duration_ms: u64,
    pub voice_id: String,
}

#[tauri::command]
async fn tts_synthesize(
    text: String,
    voice_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<TtsSynthesizeResult, String> {
    let voice_ref = voice_id.as_deref();
    let (samples, sample_rate) = state.tts.synthesize(&text, voice_ref).await?;
    let wav_bytes = tts::samples_f32_to_wav_bytes(&samples, sample_rate);
    let audio_base64 = base64::engine::general_purpose::STANDARD.encode(&wav_bytes);
    let duration_ms = (samples.len() as u64 * 1000) / (sample_rate as u64).max(1);
    let status = state.tts.status().await;
    Ok(TtsSynthesizeResult {
        audio_base64,
        sample_rate,
        num_samples: samples.len(),
        duration_ms,
        voice_id: status.voice_id.unwrap_or_default(),
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
// ============================================
// COMANDOS TAURI: STT — selección de micrófono
// ============================================

/// Lista los dispositivos de entrada (micrófonos) disponibles en el
/// sistema. Cada entrada incluye nombre y sample rate nativa.
#[tauri::command]
fn stt_list_microphones() -> Result<Vec<serde_json::Value>, String> {
    use cpal::traits::{DeviceTrait, HostTrait};
    let host = cpal::default_host();
    let mut out = Vec::new();
    let default_name = host
        .default_input_device()
        .and_then(|d| d.name().ok())
        .unwrap_or_default();
    for (idx, dev) in host
        .input_devices()
        .map_err(|e| format!("input_devices: {e}"))?
        .enumerate()
    {
        let name = dev.name().unwrap_or_else(|_| "?".to_string());
        let is_default = name == default_name;
        let sample_rate = dev
            .default_input_config()
            .map(|c| c.sample_rate().0)
            .unwrap_or(0);
        out.push(serde_json::json!({
            "index": idx,
            "name": name,
            "sample_rate": sample_rate,
            "is_default": is_default,
        }));
    }
    Ok(out)
}

/// Selecciona el dispositivo de entrada por nombre. Si está vacío,
/// usa el default. Se guarda en la variable de entorno `SYNAPSE_MIC_DEVICE`
/// que `stt_start` lee antes de elegir el dispositivo.
#[tauri::command]
fn stt_set_microphone(name: String) -> Result<(), String> {
    if name.is_empty() {
        std::env::remove_var("SYNAPSE_MIC_DEVICE");
    } else {
        std::env::set_var("SYNAPSE_MIC_DEVICE", &name);
    }
    Ok(())
}

/// Devuelve el nombre del micrófono configurado (o vacío).
#[tauri::command]
fn stt_get_microphone() -> String {
    std::env::var("SYNAPSE_MIC_DEVICE").unwrap_or_default()
}

// ============================================
// COMANDOS TAURI: SESIONES (FASE 2.4.B)
// ============================================

/// Una fila de sesión devuelta por `sessions.list` del gateway.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionRow {
    pub key: String,
    #[serde(default)]
    pub agent_id: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub updated_at: Option<i64>,
    #[serde(default)]
    pub has_active_run: Option<bool>,
}

/// Envía un `req` RPC y espera su `res` correspondiente, leyendo del
/// inbox de eventos en background. Esto es necesario para `sessions.list`
/// y `sessions.resolve` que solo devuelven `res`, no eventos de stream.
///
/// Estrategia: envía el `req` con un `id` único, luego hace polling
/// del inbox cada 50 ms hasta que llegue un `IncomingFrame::Response`
/// con ese `id`. Timeout por defecto: 5 segundos.
async fn send_request_and_wait(
    state: &AppState,
    method: &str,
    params: serde_json::Value,
    timeout_ms: u64,
) -> Result<serde_json::Value, String> {
    use std::time::{Duration, Instant};

    // Verificar conexión.
    if !*state.connected.lock().map_err(|e| e.to_string())? {
        return Err("No hay conexión activa con el gateway".to_string());
    }

    let req_id = next_req_id(state);

    // Enviar el req por el sink.
    {
        let mut guard = state.sink.lock().await;
        let sink = guard
            .as_mut()
            .ok_or_else(|| "sink no disponible a pesar de connected=true".to_string())?;
        let frame = RpcRequest {
            msg_type: "req",
            id: req_id.clone(),
            method: method.to_string(),
            params,
        };
        let text =
            serde_json::to_string(&frame).map_err(|e| format!("serializar {method}: {e}"))?;
        sink.send(Message::Text(text))
            .await
            .map_err(|e| format!("enviar {method}: {e}"))?;
    }

    // Esperar la res correspondiente.
    let started = Instant::now();
    let timeout = Duration::from_millis(timeout_ms);
    loop {
        if started.elapsed() > timeout {
            return Err(format!(
                "timeout de {timeout_ms}ms esperando respuesta a {method}"
            ));
        }
        // Buscar en el inbox una res con nuestro id.
        {
            let mut inbox = state.inbox.lock().expect("inbox poisoned");
            for (idx, ev) in inbox.iter().enumerate() {
                if ev.event == "__raw_res__" {
                    // Truco: cuando el inbox recibe una `res`, el evento
                    // pump ya la descartó (ver run_event_pump). Necesitamos
                    // otro mecanismo: escuchar el inbox como GatewayEvent
                    // parseado. Usamos un canal auxiliar: guardamos la
                    // res en el inbox con un evento "res" y un campo id
                    // en el payload.
                }
            }
            // Búsqueda más simple: si el evento tiene el formato `res`
            // guardado (lo detectamos por la presencia de un campo `id`
            // en el payload que coincida), lo extraemos.
            let mut found_idx = None;
            for (idx, ev) in inbox.iter().enumerate() {
                if let Some(id) = ev.payload.get("id").and_then(|v| v.as_str()) {
                    if id == req_id {
                        found_idx = Some(idx);
                        break;
                    }
                }
            }
            if let Some(idx) = found_idx {
                let ev = inbox.remove(idx);
                return Ok(ev.payload);
            }
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// Lista las sesiones activas del gateway vía `sessions.list`.
#[tauri::command]
async fn gateway_list_sessions(state: State<'_, AppState>) -> Result<Vec<SessionRow>, String> {
    let value = send_request_and_wait(
        state.inner(),
        "sessions.list",
        serde_json::json!({
            "configuredAgentsOnly": true,
            "activeMinutes": 1440,
            "includeLastMessage": false,
        }),
        5000,
    )
    .await?;
    // El gateway responde con `{ id, ok, payload: { sessions: [...], ... } }`.
    // Buscar `sessions` dentro de `payload` (con fallback a la raíz por
    // compatibilidad con gateways que envuelven diferente).
    let sessions = value
        .get("payload")
        .and_then(|p| p.get("sessions"))
        .or_else(|| value.get("sessions"))
        .and_then(|v| v.as_array())
        .ok_or_else(|| format!("respuesta sin 'sessions': {value}"))?;
    let mut rows: Vec<SessionRow> = Vec::with_capacity(sessions.len());
    for s in sessions {
        let row: SessionRow =
            serde_json::from_value(s.clone()).map_err(|e| format!("parsear SessionRow: {e}"))?;
        rows.push(row);
    }
    Ok(rows)
}

/// Resuelve un sessionKey arbitrario vía `sessions.resolve`. Devuelve
/// el sessionKey normalizado (canónico).
#[tauri::command]
async fn gateway_resolve_session(
    input: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    if input.trim().is_empty() {
        return Err("input vacío".to_string());
    }
    let value = send_request_and_wait(
        state.inner(),
        "sessions.resolve",
        serde_json::json!({
            "key": input.trim(),
            "allowMissing": true,
        }),
        5000,
    )
    .await?;
    // La respuesta es `{ id, ok, payload: { key: "..." | null, ... } }`.
    let payload_obj = value.get("payload");
    let key = payload_obj
        .and_then(|p| p.get("key"))
        .or_else(|| value.get("key"))
        .and_then(|v| v.as_str());
    if let Some(key) = key {
        if let Ok(mut sk) = state.session_key.lock() {
            *sk = key.to_string();
        }
        Ok(key.to_string())
    } else {
        Err(format!(
            "el gateway no pudo resolver \"{input}\": {}",
            payload_obj
                .and_then(|p| p.get("error"))
                .or_else(|| value.get("error"))
                .and_then(|v| v.as_str())
                .unwrap_or("sin error detallado")
        ))
    }
}

// ============================================
// COMANDOS TAURI: STT (FASE 2.4.C) — funcional end-to-end
// ============================================
//
// Esta fase añade:
//   - Captura de audio del micrófono con `cpal` (16 kHz mono PCM f32).
//   - Resampling lineal del audio entrante (44.1/48 kHz → 16 kHz).
//   - Reconocimiento en streaming con sherpa-onnx Zipformer.
//   - Emisión de eventos `stt:partial` al frontend durante el dictado.
//   - Detección de endpoint (silencio al final de la frase) que detiene
//     la captura automáticamente.
//   - Binario CLI `--cli-test-stt` para validar sin GUI.
//
// Requiere:
//   - libasound2-dev en Linux (ya instalado en esta VM).
//   - Modelo sherpa-onnx-streaming-zipformer-es (descargado por
//     `SttEngine::set_model` la primera vez, ~150 MB).

/// Devuelve el catálogo de modelos STT disponibles.
#[tauri::command]
fn stt_list_models() -> Vec<SttModelSpec> {
    STT_MODEL_CATALOG.to_vec()
}

/// Devuelve el estado actual del motor STT.
#[tauri::command]
async fn stt_status_cmd(state: State<'_, AppState>) -> Result<SttStatus, String> {
    Ok(state.stt.status().await)
}

/// Carga el modelo STT indicado (descargándolo si es necesario).
#[tauri::command]
async fn stt_set_model(model_id: String, state: State<'_, AppState>) -> Result<SttStatus, String> {
    state.stt.set_model(&model_id).await
}

/// Inicia una sesión de dictado por voz.
///
/// Arquitectura (FASE 2.4.C final, v7):
///   - El callback de cpal **solo** envía samples a un canal `mpsc`
///     bloqueante (mínimo, sin riesgo de crashear el WebProcess).
///   - Un hilo dedicado consume ese canal y hace el reconocimiento
///     (accept_waveform, get_result, endpoint).
///   - `audio_stream` se guarda en `AtomicPtr` global.
///   - El callback y el hilo comparten el `recognizer`+`stream` vía
///     un `Arc<Mutex<...>>` (mutex estándar, no async).
#[tauri::command]
async fn stt_start(
    model_id: Option<String>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

    // FASE 3 fix: si no se pasa model_id, leer el modelo configurado en
    // settings. Si el modelo en settings no está disponible en el bundle
    // y no está descargado localmente, usar el default (whisper-base) que
    // SÍ está en el bundle. Esto evita descargar 350MB desde internet en
    // PCs sin conexión o con conexión lenta.
    let id = if let Some(mid) = model_id {
        if mid.trim().is_empty() {
            // String vacío: leer de settings o default.
            state
                .settings
                .lock()
                .map(|s| {
                    if s.stt_model_id.is_empty() {
                        DEFAULT_STT_MODEL_ID.to_string()
                    } else {
                        s.stt_model_id.clone()
                    }
                })
                .unwrap_or_else(|_| DEFAULT_STT_MODEL_ID.to_string())
        } else {
            mid
        }
    } else {
        // None: leer de settings o default.
        state
            .settings
            .lock()
            .map(|s| {
                if s.stt_model_id.is_empty() {
                    DEFAULT_STT_MODEL_ID.to_string()
                } else {
                    s.stt_model_id.clone()
                }
            })
            .unwrap_or_else(|_| DEFAULT_STT_MODEL_ID.to_string())
    };

    // Verificar si el modelo ya está descargado localmente o en el bundle.
    // Si no está disponible offline, usar el default (whisper-base) que
    // está garantizado en el bundle.
    use stt::stt_model_dir;
    let model_dir = stt_model_dir(&id);
    let model_available_offline = model_dir
        .as_ref()
        .map(|d| {
            d.join("tokens.txt").exists()
                || d.join("tiny-tokens.txt").exists()
                || d.join("base-tokens.txt").exists()
                || std::fs::read_dir(d)
                    .ok()
                    .map(|entries| {
                        entries.flatten().any(|e| {
                            e.file_name()
                                .to_str()
                                .map(|n| n.ends_with("-tokens.txt"))
                                .unwrap_or(false)
                        })
                    })
                    .unwrap_or(false)
        })
        .unwrap_or(false);
    // Verificar si está en el bundle.
    let in_bundle = |mid: &str| -> bool {
        use std::sync::atomic::AtomicPtr;
        use std::sync::atomic::Ordering;
        // BUNDLE_RESOURCE_DIR es OnceLock, ya inicializado en setup.
        // No podemos accederlo aquí directamente, pero podemos verificar
        // si el directorio del modelo existe.
        if let Some(d) = &model_dir {
            return d.exists();
        }
        false
    };
    if !model_available_offline && id != DEFAULT_STT_MODEL_ID {
        // El modelo no está descargado y no es el default. Intentar
        // con el default (whisper-base) que está en el bundle.
        info!(
            "[stt] modelo '{}' no disponible offline, usando default '{}' (bundle)",
            id, DEFAULT_STT_MODEL_ID
        );
        // Actualizar la variable id.
        let id = DEFAULT_STT_MODEL_ID.to_string();
        info!("[stt] modelo seleccionado: {}", id);
        state.stt.set_model(&id).await?;
    } else {
        info!("[stt] modelo seleccionado: {}", id);
        state.stt.set_model(&id).await?;
    }

    // Cerrar stream anterior si existe.
    unsafe {
        let prev = STT_AUDIO_STREAM.swap(ptr::null_mut(), Ordering::AcqRel);
        if !prev.is_null() {
            let _ = Box::from_raw(prev as *mut cpal::Stream);
        }
    }

    // Obtener el handle del motor. Si es Whisper (offline), el comportamiento
    // es diferente: acumulamos audio y al detectar silencio procesamos todo.
    let handle = state
        .stt
        .handle()
        .await
        .ok_or_else(|| "modelo STT no cargado".to_string())?;
    let engine_kind = handle.engine_kind;
    info!(
        "[stt] motor = {:?}, modelo = {}",
        engine_kind, handle.model_id
    );

    let host = cpal::default_host();
    // Si el usuario seleccionó un micrófono específico, lo buscamos
    // por nombre. Si no, usamos el default del sistema.
    let mic_pref = std::env::var("SYNAPSE_MIC_DEVICE").ok();
    let device = if let Some(pref_name) = mic_pref.as_deref() {
        if !pref_name.is_empty() {
            let mut found = None;
            for dev in host
                .input_devices()
                .map_err(|e| format!("input_devices: {e}"))?
            {
                if let Ok(name) = dev.name() {
                    if name == pref_name {
                        found = Some(dev);
                        break;
                    }
                }
            }
            match found {
                Some(d) => d,
                None => {
                    info!("[stt] micrófono preferido '{pref_name}' no encontrado, usando default");
                    host.default_input_device().ok_or_else(|| {
                        format!("micrófono preferido '{pref_name}' no existe y no hay default")
                    })?
                }
            }
        } else {
            host.default_input_device()
                .ok_or_else(|| "no hay dispositivo de entrada (micrófono) disponible".to_string())?
        }
    } else {
        host.default_input_device()
            .ok_or_else(|| "no hay dispositivo de entrada (micrófono) disponible".to_string())?
    };
    let device_name = device.name().unwrap_or_else(|_| "?".to_string());
    info!("[stt] usando micrófono: {device_name}");

    let supported = device
        .default_input_config()
        .map_err(|e| format!("config de entrada: {e}"))?;
    let native_sr = supported.sample_rate().0;
    info!("[stt] sample_rate nativa = {native_sr} Hz");

    // Canal bloqueante para samples: callback → hilo de reconocimiento.
    // Usamos mpsc síncrono (no async) porque el callback de cpal es
    // síncrono y queremos cero overhead. `try_send` en el callback
    // nunca bloquea (si el buffer está lleno, descartamos samples).
    let (samples_tx, samples_rx) = mpsc::sync_channel::<Vec<f32>>(64);

    // Hilo dedicado para reconocimiento. Mantiene vivo el recognizer
    // y el stream. Sale cuando el canal se cierre (samples_tx.drop()).
    let app_for_recognizer = app.clone();
    let (stop_tx, stop_rx) = mpsc::channel::<()>();
    let recognizer_thread = std::thread::Builder::new()
        .name("stt-recognizer".into())
        .spawn(move || {
            debug!("[stt] hilo de reconocimiento activo");
            let mut chunks_processed: u64 = 0;
            let mut total_samples: u64 = 0;
            // Para Whisper (offline): acumulamos samples hasta que el usuario
            // detenga (stt_stop) o llegue silencio. Como no tenemos VAD,
            // simplemente procesamos todos los samples al recibir stt_stop.
            let mut offline_buffer: Vec<f32> = Vec::new();
            // Crear el stream/recognizer según el motor.
            let mut online_stream = match engine_kind {
                SttEngineKind::StreamingZipformer => {
                    match handle.online_recognizer_clone_blocking() {
                        Some(rec) => {
                            let stream = rec.create_stream();
                            Some((rec, stream))
                        }
                        None => {
                            error!("[stt] no se pudo crear OnlineRecognizer");
                            return;
                        }
                    }
                }
                SttEngineKind::OfflineWhisper => None,
            };
            while let Ok(samples) = samples_rx.recv() {
                // Si llegó stop, salir.
                if stop_rx.try_recv().is_ok() {
                    debug!("[stt] hilo de reconocimiento: stop recibido");
                    break;
                }
                chunks_processed += 1;
                total_samples += samples.len() as u64;
                if chunks_processed % 50 == 1 {
                    info!(
                        "[stt] diagnóstico: {} chunks, {} samples procesados",
                        chunks_processed, total_samples
                    );
                }
                // Resamplear.
                let samples_16k = if native_sr == 16000 {
                    samples
                } else {
                    resample_linear(&samples, native_sr, 16000)
                };
                match engine_kind {
                    SttEngineKind::StreamingZipformer => {
                        let (recognizer, stream) =
                            online_stream.as_mut().expect("online_stream inicializado");
                        // Alimentar al recognizer.
                        stream.accept_waveform(16000, &samples_16k);
                        // CRÍTICO: decodificar mientras haya frames listos. Sin
                        // este bucle, el modelo carga los samples en el buffer
                        // interno pero nunca produce resultados (get_result
                        // siempre retorna None o texto vacío).
                        while recognizer.is_ready(stream) {
                            recognizer.decode(stream);
                        }
                        // Resultado parcial.
                        if let Some(result) = recognizer.get_result(stream) {
                            let text = result.text.trim().to_string();
                            if !text.is_empty() {
                                let _ = app_for_recognizer.emit(
                                    "stt:partial",
                                    serde_json::json!({"text": text, "is_final": false}),
                                );
                            }
                            // Endpoint.
                            if recognizer.is_endpoint(stream) {
                                let final_text = recognizer
                                    .get_result(stream)
                                    .map(|r| r.text.trim().to_string())
                                    .unwrap_or_default();
                                let _ = app_for_recognizer.emit(
                                    "stt:final",
                                    serde_json::json!({"text": final_text, "is_final": true}),
                                );
                                info!("[stt] endpoint detectado, texto: {final_text}");
                                recognizer.reset(stream);
                            }
                        }
                    }
                    SttEngineKind::OfflineWhisper => {
                        // Whisper offline: solo acumulamos. La transcripción
                        // ocurre al final (cuando llega stop o cuando se
                        // detecta silencio tras N segundos sin audio).
                        offline_buffer.extend_from_slice(&samples_16k);
                    }
                }
            }
            // Si es offline, transcribir ahora todo lo acumulado.
            if engine_kind == SttEngineKind::OfflineWhisper {
                // Usar el `Arc<OfflineRecognizer>` cacheado para evitar
                // recrearlo (ahorra ~500 ms). Si por alguna razón no
                // está cacheado, caer al fallback.
                let recognizer_opt = handle.offline_recognizer_arc_blocking();
                if let Some(recognizer_arc) = recognizer_opt {
                    if !offline_buffer.is_empty() {
                        info!(
                            "[stt] Whisper offline: transcribiendo {} samples",
                            offline_buffer.len()
                        );
                        let stream = recognizer_arc.create_stream();
                        stream.accept_waveform(16000, &offline_buffer);
                        recognizer_arc.decode(&stream);
                        if let Some(result) = stream.get_result() {
                            let text = result.text.trim().to_string();
                            if !text.is_empty() {
                                // Guardar la transcripción en la static para
                                // que stt_stop la lea después.
                                if let Ok(mut guard) = LAST_TRANSCRIPTION.lock() {
                                    *guard = text.clone();
                                }
                                // Emitir evento al frontend.
                                let _ = app_for_recognizer.emit(
                                    "stt:final",
                                    serde_json::json!({"text": text, "is_final": true}),
                                );
                                info!("[stt] Whisper: {}", text);
                            } else {
                                info!("[stt] Whisper: transcripción vacía");
                            }
                        } else {
                            info!("[stt] Whisper: sin resultado");
                        }
                    } else {
                        info!("[stt] Whisper offline: buffer vacío, nada que transcribir");
                    }
                }
            }
            debug!("[stt] hilo de reconocimiento terminado");
        })
        .map_err(|e| format!("crear hilo de reconocimiento: {e}"))?;

    let stream_config = cpal::StreamConfig {
        channels: 1,
        sample_rate: cpal::SampleRate(native_sr),
        buffer_size: cpal::BufferSize::Default,
    };

    // El callback SOLO envía samples. No toca el recognizer ni
    // hace operaciones IPC. Esto evita que un crash en el
    // reconocimiento mate al WebProcess entero.
    // Importante: clonamos samples_tx para que el callback reciba su
    // propia copia y la original siga viva al final de stt_start.
    // Si movemos samples_tx al callback, se dropea al construir el
    // stream y el canal mpsc se cierra inmediatamente (el hilo de
    // reconocimiento termina sin haber recibido un solo sample).
    let samples_tx_for_callback = samples_tx.clone();
    let audio_stream = device
        .build_input_stream(
            &stream_config,
            move |data: &[f32], _info: &cpal::InputCallbackInfo| {
                // try_send nunca bloquea. Si el buffer está lleno,
                // descartamos este chunk (preferible a bloquear el audio).
                let _ = samples_tx_for_callback.try_send(data.to_vec());
            },
            move |err| {
                info!("[stt] error en stream de audio: {err}");
            },
            None,
        )
        .map_err(|e| format!("abrir stream: {e}"))?;

    audio_stream
        .play()
        .map_err(|e| format!("iniciar stream: {e}"))?;

    // Guardar el stream y el stop_tx como punteros crudos en globales.
    let boxed_stream = Box::new(audio_stream);
    let raw_stream = Box::into_raw(boxed_stream) as *mut cpal::Stream as *mut ();
    let stop_tx_box = Box::new(stop_tx);
    let raw_stop_tx = Box::into_raw(stop_tx_box) as *mut mpsc::Sender<()> as *mut ();
    let recognizer_thread_box = Box::new(recognizer_thread);
    let raw_thread =
        Box::into_raw(recognizer_thread_box) as *mut std::thread::JoinHandle<()> as *mut ();
    // Guardar el samples_tx en la variable estática para que NO se
    // drope al final de este comando (de lo contrario, el canal
    // mpsc se cierra, el samples_rx del hilo retorna error y el
    // hilo termina sin haber recibido ningún sample).
    let samples_tx_box = Box::new(samples_tx);
    let raw_samples_tx =
        Box::into_raw(samples_tx_box) as *mut mpsc::SyncSender<Vec<f32>> as *mut ();
    unsafe {
        STT_AUDIO_STREAM.store(raw_stream, Ordering::Release);
        STT_STOP_TX.store(raw_stop_tx, Ordering::Release);
        STT_THREAD.store(raw_thread, Ordering::Release);
        STT_SAMPLES_TX.store(raw_samples_tx, Ordering::Release);
    }

    info!("[stt] sesión de dictado activa (modelo {id})");

    // Emitir evento a TODAS las ventanas para que sincronicen su UI.
    // La ventana del chat actualiza el botón mic, y el avatar cambia
    // de animación. Esto es crítico cuando stt_start se llama desde
    // la ventana del avatar (click simple) — sin este emit, el chat
    // no sabría que el dictado está activo.
    let _ = app.emit("stt:state", serde_json::json!({"recording": true}));
    let _ = set_avatar_state("listening".to_string(), app.clone());

    Ok(())
}

/// Detiene la sesión de dictado actual.
#[tauri::command]
async fn stt_stop(app: AppHandle) -> Result<(), String> {
    unsafe {
        // Dropear el stream (cierra el audio).
        let prev_stream = STT_AUDIO_STREAM.swap(ptr::null_mut(), Ordering::AcqRel);
        if !prev_stream.is_null() {
            let _ = Box::from_raw(prev_stream as *mut cpal::Stream);
        }
        // Dropear el samples_tx. Esto cierra el canal mpsc, el
        // samples_rx del hilo retorna error, y el hilo termina
        // limpiamente.
        let prev_tx = STT_SAMPLES_TX.swap(ptr::null_mut(), Ordering::AcqRel);
        if !prev_tx.is_null() {
            let _ = Box::from_raw(prev_tx as *mut mpsc::SyncSender<Vec<f32>>);
        }
        // Señalar stop al hilo de reconocimiento.
        let prev_stop = STT_STOP_TX.swap(ptr::null_mut(), Ordering::AcqRel);
        if !prev_stop.is_null() {
            let tx = Box::from_raw(prev_stop as *mut mpsc::Sender<()>);
            let _ = tx.send(());
            drop(tx);
        }
        // Esperar al hilo a que termine. Esto bloquea ~1.5s mientras
        // Whisper decodifica, pero es necesario para que la transcripción
        // esté disponible en LAST_TRANSCRIPTION antes de mostrar el chat.
        let prev_thread = STT_THREAD.swap(ptr::null_mut(), Ordering::AcqRel);
        if !prev_thread.is_null() {
            let handle = Box::from_raw(prev_thread as *mut std::thread::JoinHandle<()>);
            match (*handle).join() {
                Ok(()) => info!("[stt] thread terminado limpiamente"),
                Err(e) => info!("[stt] thread terminó con error: {:?}", e),
            }
        }
    }
    let _ = app.emit(
        "stt:final",
        serde_json::json!({ "text": "", "is_final": true }),
    );
    // Emitir estado a todas las ventanas.
    let _ = app.emit("stt:state", serde_json::json!({"recording": false}));
    let _ = set_avatar_state("idle".to_string(), app.clone());
    info!("[stt] sesión detenida");

    // FASE 3: después de que el thread terminó, leer la transcripción
    // y manejar auto-send desde el thread principal (seguro).
    let transcription = LAST_TRANSCRIPTION
        .lock()
        .map(|s| s.clone())
        .unwrap_or_default();
    if !transcription.is_empty() {
        // Limpiar la static.
        if let Ok(mut guard) = LAST_TRANSCRIPTION.lock() {
            *guard = String::new();
        }
        use tauri::Manager;
        if let Some(chat) = app.get_webview_window("chat") {
            // Ejecutar JS en el chat para poner el texto, verificar
            // autoSend, y enviar si está activo. Si autoSend está activo,
            // NO mostrar el chat (el mensaje ya se envió, no hay nada
            // que revisar). Si autoSend NO está activo, mostrar el chat
            // para que el usuario revise y envíe manualmente.
            let text_json =
                serde_json::to_string(&transcription).unwrap_or_else(|_| "\"\"".to_string());
            let js = format!(
                "(function() {{
                    var input = document.getElementById('message-input');
                    if (input) input.value = {};
                    var chk = document.getElementById('chk-auto-send-after-dictation');
                    if (chk && chk.checked) {{
                        // Auto-send activo: enviar sin mostrar el chat.
                        if (typeof sendMessage === 'function') {{
                            sendMessage().catch(function(e) {{
                                console.error('[auto-send]', e);
                            }});
                        }}
                    }} else {{
                        // Auto-send inactivo: mostrar el chat para
                        // que el usuario revise y envíe manualmente.
                        window.__TAURI__.core.invoke('show_chat_window').catch(function(){{}});
                    }}
                }})();",
                text_json
            );
            let _ = chat.eval(&js);
            info!("[stt] Transcripción procesada: {}", transcription);
        }
    }
    Ok(())
}

/// `cpal::Stream` activo durante el dictado. Vive en una variable
/// estática global (no en AppState) porque cpal::Stream no es Send.
static STT_AUDIO_STREAM: AtomicPtr<()> = AtomicPtr::new(ptr::null_mut());

/// `mpsc::Sender<()>` para detener el hilo de reconocimiento.
static STT_STOP_TX: AtomicPtr<()> = AtomicPtr::new(ptr::null_mut());

/// `JoinHandle` del hilo de reconocimiento (para limpiarlo).
static STT_THREAD: AtomicPtr<()> = AtomicPtr::new(ptr::null_mut());

/// Última transcripción del STT. La escribe el thread de reconocimiento
/// y la lee `stt_stop` después de que el thread termina.
static LAST_TRANSCRIPTION: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

/// `mpsc::SyncSender<Vec<f32>>` para que el callback de cpal envíe
/// samples al hilo de reconocimiento. Vive en una variable estática
/// porque si se dropea al final de `stt_start`, el `samples_rx` se
/// cierra y el hilo termina con error "hilo de reconocimiento terminado"
/// sin haber recibido ningún sample.
static STT_SAMPLES_TX: AtomicPtr<()> = AtomicPtr::new(ptr::null_mut());

/// Resampling lineal de `from_hz` a `to_hz`. Suficiente para voz humana
/// (no es alta calidad para música, pero la STT no lo nota).
fn resample_linear(input: &[f32], from_hz: u32, to_hz: u32) -> Vec<f32> {
    if from_hz == to_hz || input.is_empty() {
        return input.to_vec();
    }
    let ratio = to_hz as f64 / from_hz as f64;
    let out_len = (input.len() as f64 * ratio).ceil() as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let src_idx_f = i as f64 / ratio;
        let idx0 = src_idx_f.floor() as usize;
        let idx1 = (idx0 + 1).min(input.len() - 1);
        let t = (src_idx_f - idx0 as f64) as f32;
        let s0 = input[idx0];
        let s1 = input[idx1];
        out.push(s0 + (s1 - s0) * t);
    }
    out
}

pub fn run() {
    tauri::Builder::default()
        .on_window_event(|window, event| {
            // FASE 3: interceptar el cierre del chat con el botón (X).
            // En vez de destruir la ventana, la ocultamos. Así el avatar
            // puede volver a mostrarla con toggle_chat_window o
            // show_chat_window. Si la ventana se destruye, get_webview_window
            // retorna None y no se puede recuperar.
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "chat" {
                    api.prevent_close();
                    let _ = window.hide();
                    log::info!("[avatar] ventana chat ocultada (close interceptado)");
                } else if window.label() == "avatar" {
                    // Si el avatar se cierra por cualquier medio que no sea
                    // triple-click (ej: gestor de ventanas), cerrar toda la app.
                    use tauri::Manager;
                    log::info!("[avatar] ventana avatar cerrada — saliendo de la app");
                    window.app_handle().exit(0);
                }
            }
        })
        .setup(|app| {
            // FASE 3 distribución: inicializar el directorio de recursos
            // del bundle para que tts.rs y stt.rs puedan copiar modelos
            // desde el bundle en vez de descargarlos de internet.
            // Nota: GST_PLUGIN_PATH ya se configuró en main.rs antes
            // de que Tauri arrancara.
            use tauri::Manager;
            match app.path().resource_dir() {
                Ok(rd) => {
                    log::info!("[bundle] resource_dir: {}", rd.display());
                    tts::init_bundle_resources(rd.clone());
                    stt::init_bundle_resources(rd);
                }
                Err(e) => {
                    log::warn!("[bundle] no se pudo resolver resource_dir: {e}");
                    tts::init_bundle_resources(std::path::PathBuf::new());
                    stt::init_bundle_resources(std::path::PathBuf::new());
                }
            }
            Ok(())
        })
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            get_gateway_url,
            set_gateway_url,
            get_gateway_token,
            set_gateway_token,
            is_connected,
            check_gateway_connection,
            connect_to_gateway,
            send_message_to_gateway,
            poll_gateway_events,
            disconnect_from_gateway,
            tts_list_voices,
            tts_status,
            tts_set_voice,
            tts_synthesize,
            chat_and_speak,
            // FASE 2.4 — settings persistentes + selector de sesiones + STT.
            get_settings,
            save_settings_cmd,
            reset_settings_cmd,
            gateway_list_sessions,
            gateway_resolve_session,
            stt_list_models,
            stt_status_cmd,
            stt_set_model,
            stt_start,
            stt_stop,
            stt_list_microphones,
            stt_set_microphone,
            stt_get_microphone,
            // FASE 2.5 — caché TTS persistente y timeouts configurables.
            tts_cache_lookup,
            tts_cache_store,
            tts_cache_clear,
            tts_cache_stats,
            // FASE 3 — avatar 3D y dos ventanas.
            toggle_chat_window,
            show_chat_window,
            set_avatar_state,
            get_avatar_state,
            start_dragging,
            close_avatar_window,
            resize_avatar_window,
            show_avatar_window,
        ])
        .run(tauri::generate_context!())
        .expect("error al iniciar SynapseCortana");
}

// ============================================
// COMANDOS TAURI: SETTINGS (FASE 2.4.A)
// ============================================

/// Devuelve los settings actuales cacheados en `AppState`. Equivalente
/// a leer del disco, pero más rápido.
#[tauri::command]
fn get_settings(state: State<'_, AppState>) -> AppSettings {
    state.settings.lock().map(|s| s.clone()).unwrap_or_default()
}

/// Persiste los settings en disco y actualiza el cache en `AppState`.
/// Devuelve error legible si no se puede escribir.
#[tauri::command]
fn save_settings_cmd(settings: AppSettings, state: State<'_, AppState>) -> Result<(), String> {
    save_settings(&settings)?;
    // Actualizamos también los campos espejo en AppState para que los
    // comandos que leen `gateway_url` / `gateway_token` / `session_key`
    // vean los nuevos valores sin tener que re-leer el disco.
    if let Ok(mut url) = state.gateway_url.lock() {
        *url = settings.gateway_url.clone();
    }
    if let Ok(mut tok) = state.gateway_token.lock() {
        *tok = settings.gateway_token.clone();
    }
    if let Ok(mut sk) = state.session_key.lock() {
        *sk = settings.session_key.clone();
    }
    if let Ok(mut cache) = state.settings.lock() {
        *cache = settings;
    }
    Ok(())
}

/// Borra el archivo de settings y devuelve los defaults (también
/// aplicados al cache).
#[tauri::command]
fn reset_settings_cmd(state: State<'_, AppState>) -> Result<AppSettings, String> {
    let defaults = reset_settings_on_disk();
    if let Ok(mut url) = state.gateway_url.lock() {
        *url = defaults.gateway_url.clone();
    }
    if let Ok(mut tok) = state.gateway_token.lock() {
        *tok = defaults.gateway_token.clone();
    }
    if let Ok(mut sk) = state.session_key.lock() {
        *sk = defaults.session_key.clone();
    }
    if let Ok(mut cache) = state.settings.lock() {
        *cache = defaults.clone();
    }
    Ok(defaults)
}

// ============================================
// COMANDOS TAURI: FASE 3 — AVATAR Y DOS VENTANAS
// ============================================

/// Estado actual del avatar. Se guarda en una static para acceso
/// rapido desde cualquier comando. Los valores validos son:
/// "idle", "listening", "thinking", "speaking".
static AVATAR_STATE: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

/// Alterna la visibilidad de la ventana del chat (label="chat").
/// Si esta visible, la oculta; si esta oculta, la muestra y la trae al frente.
#[tauri::command]
fn toggle_chat_window(app: AppHandle) -> Result<bool, String> {
    use tauri::Manager;
    if let Some(window) = app.get_webview_window("chat") {
        let is_visible = window
            .is_visible()
            .map_err(|e| format!("is_visible: {e}"))?;
        if is_visible {
            window.hide().map_err(|e| format!("hide: {e}"))?;
            info!("[avatar] ventana chat oculta");
            Ok(false)
        } else {
            window.show().map_err(|e| format!("show: {e}"))?;
            window.set_focus().map_err(|e| format!("set_focus: {e}"))?;
            info!("[avatar] ventana chat mostrada");
            Ok(true)
        }
    } else {
        Err("ventana 'chat' no encontrada".to_string())
    }
}

/// Muestra la ventana del chat si está oculta. NO la oculta si ya
/// está visible (a diferencia de toggle_chat_window). Usado cuando
/// el dictado termina y hay texto que el usuario necesita ver.
/// Siempre enfoca el chat después de mostrarlo.
#[tauri::command]
fn show_chat_window(app: AppHandle) -> Result<(), String> {
    use tauri::Manager;
    if let Some(window) = app.get_webview_window("chat") {
        let is_visible = window
            .is_visible()
            .map_err(|e| format!("is_visible: {e}"))?;
        if !is_visible {
            window.show().map_err(|e| format!("show: {e}"))?;
            info!("[avatar] ventana chat mostrada (show_chat_window)");
        }
        // Siempre enfocar el chat para que el usuario pueda ver el texto.
        window.set_focus().map_err(|e| format!("set_focus: {e}"))?;
        Ok(())
    } else {
        Err("ventana 'chat' no encontrada".to_string())
    }
}

/// Redimensiona la ventana del avatar al tamaño del modelo para minimizar
/// el área transparente. Llamado desde avatar.js después de cargar el .glb.
#[tauri::command]
fn resize_avatar_window(width: u32, height: u32, app: AppHandle) -> Result<(), String> {
    use tauri::Manager;
    if let Some(window) = app.get_webview_window("avatar") {
        window
            .set_size(tauri::LogicalSize::new(width, height))
            .map_err(|e| format!("set_size: {e}"))?;
        info!("[avatar] ventana redimensionada a {}x{}", width, height);
        Ok(())
    } else {
        Err("ventana 'avatar' no encontrada".to_string())
    }
}

/// Muestra la ventana del avatar si está oculta. Usado por el auto-ocultar
/// cuando el chat recupera el foco.
#[tauri::command]
fn show_avatar_window(app: AppHandle) -> Result<(), String> {
    use tauri::Manager;
    if let Some(window) = app.get_webview_window("avatar") {
        let is_visible = window
            .is_visible()
            .map_err(|e| format!("is_visible: {e}"))?;
        if !is_visible {
            window.show().map_err(|e| format!("show: {e}"))?;
            info!("[avatar] ventana mostrada (show_avatar_window)");
        }
        Ok(())
    } else {
        Err("ventana 'avatar' no encontrada".to_string())
    }
}

/// Cambia el estado del avatar y emite un evento `avatar_state_change`
/// a la ventana del avatar para que actualice la animacion.
#[tauri::command]
fn set_avatar_state(state: String, app: AppHandle) -> Result<(), String> {
    use tauri::Manager;
    // Validar estado.
    let valid = ["idle", "listening", "thinking", "speaking"];
    if !valid.contains(&state.as_str()) {
        return Err(format!("estado invalido: {state} (validos: {valid:?})"));
    }
    // Guardar estado.
    if let Ok(mut guard) = AVATAR_STATE.lock() {
        *guard = state.clone();
    }
    // Emitir evento a la ventana del avatar.
    if let Some(avatar_window) = app.get_webview_window("avatar") {
        avatar_window
            .emit("avatar_state_change", &state)
            .map_err(|e| format!("emit: {e}"))?;
    }
    info!("[avatar] estado = {state}");
    Ok(())
}

/// Devuelve el estado actual del avatar.
#[tauri::command]
fn get_avatar_state() -> String {
    AVATAR_STATE.lock().map(|s| s.clone()).unwrap_or_default()
}

/// Inicia el arrastre nativo de la ventana del avatar.
/// El SO toma el control del movimiento de la ventana hasta que el
/// usuario suelta el botón del mouse.
#[tauri::command]
fn start_dragging(app: AppHandle) -> Result<(), String> {
    use tauri::Manager;
    if let Some(window) = app.get_webview_window("avatar") {
        window
            .start_dragging()
            .map_err(|e| format!("start_dragging: {e}"))?;
        Ok(())
    } else {
        Err("ventana 'avatar' no encontrada".to_string())
    }
}

/// Cierra la ventana del avatar y TAMBIÉN cierra el chat y sale de
/// la app. Usado por el triple-click. Sin esto, el proceso sigue
/// corriendo después de cerrar el avatar y el usuario tiene que
/// matarlo desde la terminal.
#[tauri::command]
fn close_avatar_window(app: AppHandle) -> Result<(), String> {
    use tauri::Manager;
    // Detener STT si está activo.
    unsafe {
        let prev_stream = STT_AUDIO_STREAM.swap(ptr::null_mut(), Ordering::AcqRel);
        if !prev_stream.is_null() {
            let _ = Box::from_raw(prev_stream as *mut cpal::Stream);
        }
        let prev_tx = STT_SAMPLES_TX.swap(ptr::null_mut(), Ordering::AcqRel);
        if !prev_tx.is_null() {
            let _ = Box::from_raw(prev_tx as *mut mpsc::SyncSender<Vec<f32>>);
        }
        let prev_stop = STT_STOP_TX.swap(ptr::null_mut(), Ordering::AcqRel);
        if !prev_stop.is_null() {
            let tx = Box::from_raw(prev_stop as *mut mpsc::Sender<()>);
            let _ = tx.send(());
        }
        let prev_thread = STT_THREAD.swap(ptr::null_mut(), Ordering::AcqRel);
        if !prev_thread.is_null() {
            let _ = Box::from_raw(prev_thread as *mut std::thread::JoinHandle<()>);
        }
    }
    // Cerrar ambas ventanas.
    if let Some(avatar) = app.get_webview_window("avatar") {
        let _ = avatar.close();
    }
    if let Some(chat) = app.get_webview_window("chat") {
        let _ = chat.close();
    }
    info!("[avatar] app cerrada por triple-click");
    // Salir de la app.
    app.exit(0);
    Ok(())
}

// ============================================
// CACHÉ TTS PERSISTENTE (FASE 2.5)
// ============================================
//
// Guarda cada WAV sintetizado en disco (formato <texto>|<voz> → SHA-256)
// para que las frases repetidas no se re-sinteticen. Esto reduce
// drasticamente la latencia del botón "🔊 Reproducir de nuevo" y de
// saludos/preguntas frecuentes.
//
// Estructura en disco:
//
//   ~/.config/synapse-cortana/tts-cache/
//       ├── a1b2c3...wav    ← 22050 Hz, mono, 16-bit PCM (el mismo
//       │                      WAV que devuelve `tts_synthesize`).
//       │                      Tamaño típico: ~30 KB por segundo de audio.
//       └── ...

/// Ruta raíz del caché TTS. Crea el directorio si no existe.
fn tts_cache_dir() -> Option<PathBuf> {
    let base = directories::ProjectDirs::from("ai", "openclaw", "synapse-cortana")?;
    let dir = base.config_dir().join("tts-cache");
    if !dir.exists() {
        let _ = std::fs::create_dir_all(&dir);
    }
    Some(dir)
}

/// Calcula la clave del caché: SHA-256 de `<voz>\n<texto>` (texto en
/// UTF-8). Devuelve un nombre de archivo seguro (hex de 64 chars).
fn tts_cache_key(text: &str, voice_id: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(voice_id.as_bytes());
    hasher.update(b"\n");
    hasher.update(text.as_bytes());
    let hash = hasher.finalize();
    format!("{:x}.wav", hash)
}

/// Busca en el caché un WAV pre-sintetizado para `<text, voice_id>`.
/// Devuelve `Some(audio_base64)` si hay match, `None` si no.
#[tauri::command]
fn tts_cache_lookup(text: String, voice_id: String) -> Option<String> {
    let dir = tts_cache_dir()?;
    let path = dir.join(tts_cache_key(&text, &voice_id));
    if !path.exists() {
        return None;
    }
    match std::fs::read(&path) {
        Ok(bytes) => Some(base64::engine::general_purpose::STANDARD.encode(&bytes)),
        Err(e) => {
            warn!("[tts-cache] no pude leer {}: {e}", path.display());
            None
        }
    }
}

/// Guarda un WAV en el caché persistente. Devuelve `true` si se guardó
/// correctamente, `false` si falló (sin abortar la operación principal).
#[tauri::command]
fn tts_cache_store(text: String, voice_id: String, audio_base64: String) -> bool {
    let Some(dir) = tts_cache_dir() else {
        return false;
    };
    let bytes = match base64::engine::general_purpose::STANDARD.decode(audio_base64.as_bytes()) {
        Ok(b) => b,
        Err(e) => {
            warn!("[tts-cache] base64 inválido al guardar: {e}");
            return false;
        }
    };
    let path = dir.join(tts_cache_key(&text, &voice_id));
    match std::fs::write(&path, &bytes) {
        Ok(_) => {
            debug!(
                "[tts-cache] guardado {} ({} bytes)",
                path.display(),
                bytes.len()
            );
            true
        }
        Err(e) => {
            warn!("[tts-cache] no pude escribir {}: {e}", path.display());
            false
        }
    }
}

/// Borra todo el caché TTS. Devuelve el número de archivos eliminados.
#[tauri::command]
fn tts_cache_clear() -> usize {
    let Some(dir) = tts_cache_dir() else {
        return 0;
    };
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return 0,
    };
    let mut count = 0;
    for entry in entries.flatten() {
        if std::fs::remove_file(entry.path()).is_ok() {
            count += 1;
        }
    }
    info!("[tts-cache] {} archivos eliminados", count);
    count
}

/// Devuelve el número de entradas en el caché y el tamaño total en bytes.
#[tauri::command]
fn tts_cache_stats() -> serde_json::Value {
    let mut count = 0;
    let mut total_bytes: u64 = 0;
    if let Some(dir) = tts_cache_dir() {
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                if let Ok(meta) = entry.metadata() {
                    if meta.is_file() {
                        count += 1;
                        total_bytes += meta.len();
                    }
                }
            }
        }
    }
    serde_json::json!({
        "count": count,
        "total_bytes": total_bytes,
        "total_mb": (total_bytes as f64) / 1_048_576.0,
    })
}
