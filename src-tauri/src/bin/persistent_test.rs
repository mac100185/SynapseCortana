// src-tauri/src/bin/persistent_test.rs
// Hace el handshake EXACTAMENTE como lo hace synapse-cortana:
// - Genera o carga la clave Ed25519 persistente en ~/.config/...
// - Firma el payload v2 con el nonce real del server
// - Imprime la respuesta completa
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine as _;
use ed25519_dalek::{Signer, SigningKey};
use futures_util::{SinkExt, StreamExt};
use sha2::{Digest, Sha256};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, http::HeaderValue, Message},
};

fn key_path() -> Option<PathBuf> {
    let base = directories::ProjectDirs::from("ai", "openclaw", "synapse-cortana")?;
    Some(base.config_dir().join("device.key"))
}

fn load_or_create() -> Result<SigningKey, String> {
    let path = key_path().ok_or("no config dir")?;
    if path.exists() {
        let pem = std::fs::read_to_string(&path).map_err(|e| format!("read: {e}"))?;
        use ed25519_dalek::pkcs8::DecodePrivateKey;
        return SigningKey::from_pkcs8_pem(&pem).map_err(|e| format!("decode: {e}"));
    }
    use ed25519_dalek::pkcs8::EncodePrivateKey;
    let mut csprng = rand::rngs::OsRng;
    let sk = SigningKey::generate(&mut csprng);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    let pem = sk
        .to_pkcs8_pem(Default::default())
        .map_err(|e| format!("enc: {e}"))?;
    std::fs::write(&path, pem.as_bytes()).map_err(|e| format!("write: {e}"))?;
    Ok(sk)
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let token = "d86bbd15f647a311ee96322cc579546705023a63813fa20c";
    let sk = load_or_create().expect("load key");
    let pub_bytes = sk.verifying_key().to_bytes();
    let device_id = {
        let mut h = Sha256::new();
        h.update(pub_bytes);
        hex::encode(h.finalize())
    };
    let pub_b64url = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(pub_bytes);
    eprintln!("[test] key_path = {:?}", key_path());
    eprintln!("[test] device_id = {device_id}");
    eprintln!("[test] publicKey = {pub_b64url}");

    let mut request = "ws://127.0.0.1:18789/".into_client_request().expect("URL");
    request.headers_mut().insert(
        "User-Agent",
        HeaderValue::from_static("synapse-cortana/0.1.0"),
    );
    let (mut ws, _) = match connect_async(request).await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[test] FAIL connect: {e}");
            std::process::exit(1);
        }
    };
    eprintln!("[test] WS conectado");

    let nonce = loop {
        let msg = ws.next().await.expect("stream").expect("msg");
        let raw = match msg {
            Message::Text(t) => t.to_string(),
            Message::Binary(b) => String::from_utf8(b.to_vec()).unwrap(),
            Message::Ping(_) | Message::Pong(_) | Message::Frame(_) => continue,
            Message::Close(c) => {
                eprintln!("[test] cerrado: {c:?}");
                std::process::exit(1);
            }
        };
        let v: serde_json::Value = serde_json::from_str(&raw).expect("json");
        if v["event"] == "connect.challenge" {
            break v["payload"]["nonce"].as_str().unwrap().to_string();
        }
    };
    eprintln!("[test] nonce = {nonce}");

    let signed_at_ms: i64 = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    let payload = format!(
        "v2|{device_id}|gateway-client|backend|operator|operator.read,operator.write|{signed_at_ms}|{token}|{nonce}"
    );
    eprintln!("[test] payload = {payload}");
    let sig = sk.sign(payload.as_bytes());
    let sig_b64url = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(sig.to_bytes());

    let connect = serde_json::json!({
        "type": "req",
        "id": "synapse-cortana-test",
        "method": "connect",
        "params": {
            "minProtocol": 3,
            "maxProtocol": 4,
            "client": {
                "id": "gateway-client",
                "version": "0.1.0",
                "platform": "linux",
                "mode": "backend",
            },
            "role": "operator",
            "scopes": ["operator.read", "operator.write"],
            "caps": [],
            "commands": [],
            "permissions": {},
            "locale": "es-ES",
            "userAgent": "synapse-cortana/0.1.0",
            "auth": {"token": token},
            "device": {
                "id": device_id,
                "publicKey": pub_b64url,
                "signature": sig_b64url,
                "signedAt": signed_at_ms,
                "nonce": nonce,
            }
        }
    });
    let text = serde_json::to_string(&connect).unwrap();
    ws.send(Message::Text(text)).await.expect("send");

    let resp_raw = tokio::time::timeout(std::time::Duration::from_secs(10), ws.next()).await;
    match resp_raw {
        Ok(Some(Ok(Message::Text(t)))) => {
            eprintln!("[test] RESPUESTA:\n{t}");
        }
        _ => eprintln!("[test] sin respuesta"),
    }
}
