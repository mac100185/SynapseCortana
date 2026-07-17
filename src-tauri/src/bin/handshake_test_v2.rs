// src-tauri/src/bin/handshake_test_v2.rs
// Calcula la firma DENTRO de try_handshake, con el nonce real del servidor.
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine as _;
use ed25519_dalek::{Signature, Signer, SigningKey};
use futures_util::{SinkExt, StreamExt};
use sha2::{Digest, Sha256};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, http::HeaderValue, Message},
};

const TOKEN: &str = "d86bbd15f647a311ee96322cc579546705023a63813fa20c";

async fn try_handshake(
    label: &str,
    signing_key: &SigningKey,
    device_id: String,
    pub_b64url: String,
    signed_at_ms: i64,
) -> String {
    let mut request = "ws://127.0.0.1:18789/".into_client_request().expect("URL");
    request.headers_mut().insert(
        "User-Agent",
        HeaderValue::from_static("synapse-cortana-test/0.1.0"),
    );
    let (mut ws, _) = match connect_async(request).await {
        Ok(c) => c,
        Err(e) => return format!("[{}] FAIL connect: {}", label, e),
    };

    // 1) Recibir challenge y extraer el nonce REAL del server
    let server_nonce = loop {
        let msg = ws.next().await.expect("stream").expect("ws message");
        let raw = match msg {
            Message::Text(t) => t.to_string(),
            Message::Binary(b) => String::from_utf8(b.to_vec()).unwrap(),
            Message::Ping(_) | Message::Pong(_) | Message::Frame(_) => continue,
            Message::Close(c) => return format!("[{}] cerrado: {:?}", label, c),
        };
        let v: serde_json::Value = serde_json::from_str(&raw).expect("json");
        if v["event"] == "connect.challenge" {
            break v["payload"]["nonce"].as_str().unwrap().to_string();
        }
    };
    eprintln!("[{}] server_nonce={}", label, server_nonce);

    // 2) Construir y firmar el payload CON EL NONCE REAL
    let payload = match label {
        "V2" => format!(
            "v2|{device_id}|gateway-client|backend|operator|operator.read,operator.write|{signed_at_ms}|{token}|{server_nonce}",
            device_id = device_id, token = TOKEN, server_nonce = server_nonce, signed_at_ms = signed_at_ms
        ),
        "V3+token" => format!(
            "v3|{device_id}|gateway-client|backend|operator|operator.read,operator.write|{signed_at_ms}|{token}|{server_nonce}|linux|linux",
            device_id = device_id, token = TOKEN, server_nonce = server_nonce, signed_at_ms = signed_at_ms
        ),
        "V3+empty_token" => format!(
            "v3|{device_id}|gateway-client|backend|operator|operator.read,operator.write|{signed_at_ms}||{server_nonce}|linux|linux",
            device_id = device_id, server_nonce = server_nonce, signed_at_ms = signed_at_ms
        ),
        _ => return format!("[{}] label desconocido", label),
    };
    eprintln!("[{}] payload={}", label, payload);

    let sig: Signature = signing_key.sign(payload.as_bytes());
    let sig_b64url = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(sig.to_bytes());

    // 3) Enviar connect
    let connect = serde_json::json!({
        "type": "req",
        "id": format!("test-{}", label),
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
            "userAgent": "synapse-cortana-test/0.1.0",
            "auth": {"token": TOKEN},
            "device": {
                "id": device_id,
                "publicKey": pub_b64url,
                "signature": sig_b64url,
                "signedAt": signed_at_ms,
                "nonce": server_nonce,
            }
        }
    });
    let text = serde_json::to_string(&connect).unwrap();
    ws.send(Message::Text(text.into())).await.expect("send");

    let resp_raw = tokio::time::timeout(std::time::Duration::from_secs(8), ws.next()).await;
    match resp_raw {
        Ok(Some(Ok(Message::Text(t)))) => {
            let v: serde_json::Value = serde_json::from_str(&t).expect("json");
            if v["ok"] == serde_json::Value::Bool(true) {
                format!("[{}] ✅ HANDSHAKE OK", label)
            } else {
                let code = v["error"]["details"]["code"].as_str().unwrap_or("?");
                let reason = v["error"]["details"]["reason"].as_str().unwrap_or("?");
                format!("[{}] ❌ RECHAZADO: code={} reason={}", label, code, reason)
            }
        }
        _ => format!("[{}] ❌ sin respuesta", label),
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let mut csprng = rand::rngs::OsRng;
    let signed_at_ms: i64 = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;

    // Probar V2
    let sk_v2 = SigningKey::generate(&mut csprng);
    let pb_v2 = sk_v2.verifying_key().to_bytes();
    let did_v2 = {
        let mut h = Sha256::new();
        h.update(pb_v2);
        hex::encode(h.finalize())
    };
    let pbk_v2 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(pb_v2);
    println!(
        "{}",
        try_handshake("V2", &sk_v2, did_v2, pbk_v2, signed_at_ms).await
    );

    // Probar V3 con token
    let sk_v3 = SigningKey::generate(&mut csprng);
    let pb_v3 = sk_v3.verifying_key().to_bytes();
    let did_v3 = {
        let mut h = Sha256::new();
        h.update(pb_v3);
        hex::encode(h.finalize())
    };
    let pbk_v3 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(pb_v3);
    println!(
        "{}",
        try_handshake("V3+token", &sk_v3, did_v3, pbk_v3, signed_at_ms).await
    );

    // Probar V3 con token vacío
    let sk_v3e = SigningKey::generate(&mut csprng);
    let pb_v3e = sk_v3e.verifying_key().to_bytes();
    let did_v3e = {
        let mut h = Sha256::new();
        h.update(pb_v3e);
        hex::encode(h.finalize())
    };
    let pbk_v3e = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(pb_v3e);
    println!(
        "{}",
        try_handshake("V3+empty_token", &sk_v3e, did_v3e, pbk_v3e, signed_at_ms).await
    );
}
