// Bin de prueba: conexión con `client.id = "webchat-ui"` + `client.mode = "ui"`
// para ver si el gateway emite los eventos del LLM en ese modo.
//
// Uso:
//   cargo run --release --bin chat_and_speak_ui_test -- \
//     --url http://127.0.0.1:18789 \
//     --token <TOKEN> \
//     --message "Hola" \
//     --silence-ms 8000 --overall-ms 30000
//
// Loguea TODOS los eventos que llegan (event, payload keys, fragmentos
// de texto). Útil para diagnosticar el modo `ui`.

use futures_util::{SinkExt, StreamExt};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, http::HeaderValue, Message},
};

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let url = arg_value(&args, "--url").unwrap_or_else(|| "http://127.0.0.1:18789".to_string());
    let token = arg_value(&args, "--token");
    let message =
        arg_value(&args, "--message").unwrap_or_else(|| "Hola, preséntate brevemente".to_string());
    let silence_ms: u64 = arg_value(&args, "--silence-ms")
        .and_then(|s| s.parse().ok())
        .unwrap_or(5000);
    let overall_ms: u64 = arg_value(&args, "--overall-ms")
        .and_then(|s| s.parse().ok())
        .unwrap_or(30_000);

    eprintln!("[ui-test] URL      = {url}");
    eprintln!("[ui-test] mensaje  = {message}");
    eprintln!("[ui-test] silencio = {silence_ms} ms");
    eprintln!("[ui-test] timeout  = {overall_ms} ms");

    // Identidad persistente (la misma del CLI principal).
    let device = match synapse_cortana::DeviceIdentity::load_or_create() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("[ui-test] ERROR creando identidad: {e}");
            std::process::exit(1);
        }
    };
    let device_id = device.device_id();
    let public_key_b64url = device.public_key_base64url();
    eprintln!("[ui-test] device.id = {device_id}");

    let mut ws_url = url.clone();
    if ws_url.starts_with("https://") {
        ws_url = ws_url.replacen("https://", "wss://", 1);
    } else if ws_url.starts_with("http://") {
        ws_url = ws_url.replacen("http://", "ws://", 1);
    }
    ws_url = ws_url.trim_end_matches('/').to_string();

    let mut request = ws_url.clone().into_client_request().expect("URL inválida");
    request.headers_mut().insert(
        "User-Agent",
        HeaderValue::from_static("synapse-cortana/0.1.0"),
    );
    // El gateway exige un Origin permitido en `controlUi.allowedOrigins`
    // para clientes UI. Usamos el `url` (HTTP) como origin.
    request.headers_mut().insert(
        "Origin",
        HeaderValue::from_str(&url).expect("Origin inválido"),
    );

    let (mut ws, _response) = match connect_async(request).await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[ui-test] ERROR conectando: {e}");
            std::process::exit(1);
        }
    };

    // Esperar challenge.
    let nonce = loop {
        let msg = match ws.next().await {
            Some(Ok(m)) => m,
            _ => {
                eprintln!("[ui-test] ERROR challenge");
                std::process::exit(1);
            }
        };
        let raw = match msg {
            Message::Text(t) => t.to_string(),
            Message::Binary(b) => String::from_utf8(b.to_vec()).unwrap_or_default(),
            Message::Ping(_) | Message::Pong(_) | Message::Frame(_) => continue,
            Message::Close(c) => {
                eprintln!("[ui-test] gateway cerró: {c:?}");
                std::process::exit(1);
            }
        };
        let v: serde_json::Value = match serde_json::from_str(&raw) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if v["event"] == "connect.challenge" {
            break v["payload"]["nonce"]
                .as_str()
                .unwrap_or_default()
                .to_string();
        }
    };
    eprintln!("[ui-test] challenge OK, nonce = {nonce}");

    // MODOS A PROBAR:
    //   1. webchat-ui / ui  (control-ui equivalente para webchat)
    //   2. control-ui / ui  (control-ui original)
    //   3. tui / ui         (terminal ui)
    //
    // Empezamos con webchat-ui/ui.
    let platform = match std::env::consts::OS {
        "macos" => "darwin",
        "windows" => "win32",
        other => other,
    };
    let client_id = "webchat-ui"; // probar también: control-ui, tui
    let client_mode = "ui";
    let role = "operator";
    let scopes: Vec<String> = vec!["operator.read".to_string(), "operator.write".to_string()];
    let signed_at_ms: i64 = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    let resolved_token = token.unwrap_or_default();

    let (_payload, signature_b64url) = device.sign_v2(
        client_id,
        client_mode,
        role,
        &scopes,
        &resolved_token,
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
    if !resolved_token.is_empty() {
        connect_params["auth"] = serde_json::json!({ "token": resolved_token });
    }
    let connect_frame = serde_json::json!({
        "type": "req",
        "id": "ui-test-connect",
        "method": "connect",
        "params": connect_params
    });
    ws.send(Message::Text(connect_frame.to_string()))
        .await
        .expect("enviar connect");

    // Esperar hello-ok o error.
    loop {
        let msg = match ws.next().await {
            Some(Ok(m)) => m,
            _ => {
                eprintln!("[ui-test] ERROR esperando hello-ok");
                std::process::exit(1);
            }
        };
        let raw = match msg {
            Message::Text(t) => t.to_string(),
            Message::Binary(b) => String::from_utf8(b.to_vec()).unwrap_or_default(),
            Message::Ping(_) | Message::Pong(_) | Message::Frame(_) => continue,
            Message::Close(c) => {
                eprintln!("[ui-test] gateway cerró: {c:?}");
                std::process::exit(1);
            }
        };
        let v: serde_json::Value = match serde_json::from_str(&raw) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if v["type"] == "res" {
            let ok = v["ok"] == serde_json::Value::Bool(true);
            eprintln!(
                "[ui-test] connect.ok = {ok}, payload: {}",
                serde_json::to_string(&v["payload"]).unwrap_or_default()
            );
            if !ok {
                eprintln!(
                    "[ui-test] error: {}",
                    serde_json::to_string(&v["error"]).unwrap_or_default()
                );
                std::process::exit(1);
            }
            break;
        }
    }
    eprintln!("[ui-test] hello-ok: client.id={client_id}, client.mode={client_mode}");

    // Enviar chat.send.
    let req_id = "ui-test-chat-1".to_string();
    let session_key =
        arg_value(&args, "--session-key").unwrap_or_else(|| "agent:main:main".to_string());
    // El gateway exige un `idempotencyKey` único por envío.
    let idempotency_key = format!(
        "synapse-cortana-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let chat_frame = serde_json::json!({
        "type": "req",
        "id": req_id,
        "method": "chat.send",
        "params": {
            "message": message,
            "sessionKey": session_key,
            "idempotencyKey": idempotency_key
        }
    });
    let sent_at = Instant::now();
    ws.send(Message::Text(chat_frame.to_string()))
        .await
        .expect("enviar chat.send");
    eprintln!("[ui-test] chat.send enviado (id={req_id}, sessionKey={session_key}, idempotencyKey={idempotency_key})");

    // Esperar eventos y loguear todo.
    let mut last_event_at = Instant::now();
    let mut accumulated = String::new();
    let overall_deadline = Instant::now() + Duration::from_millis(overall_ms);

    loop {
        let recv = match tokio::time::timeout(Duration::from_millis(200), ws.next()).await {
            Ok(Some(Ok(m))) => m,
            Ok(Some(Err(_))) | Ok(None) => {
                eprintln!("[ui-test] stream cerrado");
                break;
            }
            Err(_) => {
                if last_event_at.elapsed() >= Duration::from_millis(silence_ms)
                    && !accumulated.is_empty()
                {
                    break;
                }
                if Instant::now() >= overall_deadline {
                    break;
                }
                continue;
            }
        };
        let raw = match recv {
            Message::Text(t) => t.to_string(),
            Message::Binary(b) => String::from_utf8(b.to_vec()).unwrap_or_default(),
            Message::Ping(_) | Message::Pong(_) | Message::Frame(_) => continue,
            Message::Close(c) => {
                eprintln!("[ui-test] gateway cerró: {c:?}");
                break;
            }
        };
        let v: serde_json::Value = match serde_json::from_str(&raw) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let v_type = v["type"].as_str().unwrap_or("");
        if v_type == "res" {
            let id = v["id"].as_str().unwrap_or("?");
            let ok = v["ok"] == serde_json::Value::Bool(true);
            if !ok {
                eprintln!(
                    "[ui-test] res ERROR: id={id} error={}",
                    serde_json::to_string(&v["error"]).unwrap_or_default()
                );
            } else {
                eprintln!(
                    "[ui-test] res OK: id={id} keys={:?}",
                    v["payload"]
                        .as_object()
                        .map(|o| o.keys().cloned().collect::<Vec<_>>())
                        .unwrap_or_default()
                );
            }
            continue;
        }
        if v_type != "event" {
            eprintln!("[ui-test] (no-event frame: {})", v);
            continue;
        }
        let event_name = v["event"].as_str().unwrap_or("");
        let payload_summary = format!(
            "{{text_len={}, message_len={}, delta_len={}, content_len={}, keys={:?}}}",
            v["payload"]["text"].as_str().map(|s| s.len()).unwrap_or(0),
            v["payload"]["message"]
                .as_str()
                .map(|s| s.len())
                .unwrap_or(0),
            v["payload"]["delta"].as_str().map(|s| s.len()).unwrap_or(0),
            v["payload"]["content"]
                .as_str()
                .map(|s| s.len())
                .unwrap_or(0),
            v["payload"]
                .as_object()
                .map(|o| o.keys().cloned().collect::<Vec<_>>())
                .unwrap_or_default(),
        );
        eprintln!("[ui-test] EVENT: {event_name} {payload_summary}");
        for key in ["deltaText", "message", "text", "delta", "content"] {
            if let Some(s) = v["payload"][key].as_str() {
                if !s.is_empty() {
                    let preview: String = s.chars().take(120).collect();
                    eprintln!(
                        "[ui-test]   {key}={preview:?}{}",
                        if s.len() > 120 { "..." } else { "" }
                    );
                }
            }
        }
        // Acumular. El gateway v4 usa campos `deltaText` (en eventos `chat`)
        // y `message` (evento terminal). También probamos `text`/`content` por
        // compatibilidad con otros canales.
        for key in ["deltaText", "message", "text", "delta", "content"] {
            if let Some(s) = v["payload"][key].as_str() {
                if !s.is_empty() {
                    if !accumulated.is_empty()
                        && !accumulated.ends_with(char::is_whitespace)
                        && !s.starts_with(char::is_whitespace)
                    {
                        accumulated.push(' ');
                    }
                    accumulated.push_str(s);
                    last_event_at = Instant::now();
                }
            }
        }
    }
    let elapsed = sent_at.elapsed();
    eprintln!(
        "[ui-test] fin. elapsed={:.2}s, {} chars acumulados",
        elapsed.as_secs_f32(),
        accumulated.len()
    );
    if !accumulated.is_empty() {
        eprintln!("[ui-test] texto del agente:");
        eprintln!("---");
        for line in accumulated.lines() {
            eprintln!("  {line}");
        }
        eprintln!("---");
    }
    let _ = ws.close(None).await;
    std::process::exit(if accumulated.is_empty() { 1 } else { 0 });
}

fn arg_value(args: &[String], flag: &str) -> Option<String> {
    let pos = args.iter().position(|a| a == flag)?;
    args.get(pos + 1).cloned()
}
