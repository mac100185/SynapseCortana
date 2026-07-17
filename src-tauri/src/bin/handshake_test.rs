// src-tauri/src/bin/handshake_test.rs
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine as _;
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use futures_util::{SinkExt, StreamExt};
use sha2::{Digest, Sha256};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, http::HeaderValue, Message},
};

#[tokio::main(flavor = "current_thread")]
async fn main() {
    eprintln!("[test] Iniciando handshake de prueba contra ws://127.0.0.1:18789/");

    // 1) Generar clave Ed25519
    let mut csprng = rand::rngs::OsRng;
    let signing_key = SigningKey::generate(&mut csprng);
    let pub_bytes = signing_key.verifying_key().to_bytes();
    let device_id = {
        let mut h = Sha256::new();
        h.update(pub_bytes);
        hex::encode(h.finalize())
    };
    let pub_b64url = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(pub_bytes);
    eprintln!("[test] device.id = {}...", &device_id[..16]);

    // 2) Conectar WS
    let url = "ws://127.0.0.1:18789/";
    let mut request = url.into_client_request().expect("URL válida");
    request.headers_mut().insert(
        "User-Agent",
        HeaderValue::from_static("synapse-cortana-handshake-test/0.1.0"),
    );

    let (mut ws, _) = match connect_async(request).await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[test] FAIL connect: {e}");
            std::process::exit(1);
        }
    };
    eprintln!("[test] WS conectado");

    // 3) Recibir challenge
    let nonce = loop {
        let msg = ws.next().await.expect("stream").expect("ws message");
        let raw = match msg {
            Message::Text(t) => t.to_string(),
            Message::Binary(b) => String::from_utf8(b.to_vec()).unwrap(),
            Message::Ping(_) | Message::Pong(_) | Message::Frame(_) => continue,
            Message::Close(c) => {
                eprintln!("[test] Cerrado: {c:?}");
                std::process::exit(1);
            }
        };
        eprintln!("[test] Challenge: {raw}");
        let v: serde_json::Value = serde_json::from_str(&raw).expect("json");
        if v["event"] == "connect.challenge" {
            break v["payload"]["nonce"].as_str().unwrap().to_string();
        }
    };
    eprintln!("[test] nonce = {nonce}");

    // 4) Construir payload v3 firmado
    let token = "d86bbd15f647a311ee96322cc579546705023a63813fa20c";
    let client_id = "gateway-client";
    let client_mode = "backend";
    let role = "operator";
    let scopes = "operator.read,operator.write";
    let platform = "linux";
    let device_family = "linux";
    let signed_at_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    let payload = format!(
        "v3|{device_id}|{client_id}|{client_mode}|{role}|{scopes}|{signed_at_ms}|{token}|{nonce}|{platform}|{device_family}"
    );
    eprintln!("[test] Payload: {payload}");

    let sig: Signature = signing_key.sign(payload.as_bytes());
    let sig_b64url = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(sig.to_bytes());
    eprintln!(
        "[test] sig (primeros 40): {}...",
        &sig_b64url[..40.min(sig_b64url.len())]
    );

    // 5) Enviar connect
    let connect = serde_json::json!({
        "type": "req",
        "id": "test-1",
        "method": "connect",
        "params": {
            "minProtocol": 3,
            "maxProtocol": 4,
            "client": {
                "id": client_id,
                "version": "0.1.0",
                "platform": platform,
                "mode": client_mode,
            },
            "role": role,
            "scopes": ["operator.read", "operator.write"],
            "caps": [],
            "commands": [],
            "permissions": {},
            "locale": "es-ES",
            "userAgent": "synapse-cortana-handshake-test/0.1.0",
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
    eprintln!("[test] Enviando connect ({} bytes)...", text.len());
    ws.send(Message::Text(text.into()))
        .await
        .expect("send connect");

    // 6) Esperar respuesta
    let resp_raw = tokio::time::timeout(std::time::Duration::from_secs(10), ws.next()).await;
    match resp_raw {
        Ok(Some(Ok(Message::Text(t)))) => {
            eprintln!("[test] RESPUESTA RAW: {t}");
            let v: serde_json::Value = serde_json::from_str(&t).expect("json");
            if v["ok"] == serde_json::Value::Bool(true) {
                eprintln!("[test] ✅ HANDSHAKE OK");
                eprintln!("[test] payload.type = {}", v["payload"]["type"]);
                eprintln!("[test] protocol = {}", v["payload"]["protocol"]);
                eprintln!(
                    "[test] server.version = {}",
                    v["payload"]["server"]["version"]
                );
                eprintln!("[test] auth.role = {}", v["payload"]["auth"]["role"]);
                eprintln!("[test] auth.scopes = {:?}", v["payload"]["auth"]["scopes"]);
            } else {
                eprintln!("[test] ❌ RECHAZADO:");
                eprintln!("[test] code = {}", v["error"]["code"]);
                eprintln!("[test] message = {}", v["error"]["message"]);
                if let Some(d) = v["error"].get("details") {
                    eprintln!("[test] details = {d}");
                }
            }
        }
        _ => eprintln!("[test] ❌ No se recibió respuesta en 10s"),
    }
}
