// src-tauri/src/bin/sign_test.rs
// Construye el payload v3 y v2 del device-auth, firma con Ed25519,
// y muestra el payload en hex + la firma en hex para comparar con Python.
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine as _;
use ed25519_dalek::{Signature, Signer, SigningKey};
use sha2::{Digest, Sha256};

fn sha256_hex(data: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(data);
    hex::encode(h.finalize())
}

fn main() {
    let mut csprng = rand::rngs::OsRng;
    let signing_key = SigningKey::generate(&mut csprng);
    let pub_bytes = signing_key.verifying_key().to_bytes();
    let device_id = {
        let mut h = Sha256::new();
        h.update(pub_bytes);
        hex::encode(h.finalize())
    };
    let pub_b64url = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(pub_bytes);

    let token = "d86bbd15f647a311ee96322cc579546705023a63813fa20c";
    let client_id = "gateway-client";
    let client_mode = "backend";
    let role = "operator";
    let scopes_csv = "operator.read,operator.write";
    let nonce = "7406bc3e-d5b8-4558-b407-4dd3c0e6e797";
    let platform = "linux";
    let device_family = "linux";
    let signed_at_ms: i64 = 1781752085528;

    // V3 payload (mi implementación actual)
    let payload_v3 = format!(
        "v3|{device_id}|{client_id}|{client_mode}|{role}|{scopes_csv}|{signed_at_ms}|{token}|{nonce}|{platform}|{device_family}"
    );
    let sig_v3: Signature = signing_key.sign(payload_v3.as_bytes());

    // V2 payload (alternativa más simple, sin platform/deviceFamily)
    let payload_v2 = format!(
        "v2|{device_id}|{client_id}|{client_mode}|{role}|{scopes_csv}|{signed_at_ms}|{token}|{nonce}"
    );
    let sig_v2: Signature = signing_key.sign(payload_v2.as_bytes());

    // V3 con token="" (caso donde el server lo pasaría vacío)
    let payload_v3_empty_token = format!(
        "v3|{device_id}|{client_id}|{client_mode}|{role}|{scopes_csv}|{signed_at_ms}||{nonce}|{platform}|{device_family}"
    );
    let sig_v3_empty: Signature = signing_key.sign(payload_v3_empty_token.as_bytes());

    println!("device_id:         {device_id}");
    println!("publicKey (b64url): {pub_b64url}");
    println!();
    println!("=== V3 payload (con token) ===");
    println!("TEXT: {payload_v3}");
    println!("SHA256: {}", sha256_hex(payload_v3.as_bytes()));
    println!("SIG hex: {}", hex::encode(sig_v3.to_bytes()));
    println!();
    println!("=== V3 payload (token vacío '') ===");
    println!("TEXT: {payload_v3_empty_token}");
    println!("SHA256: {}", sha256_hex(payload_v3_empty_token.as_bytes()));
    println!("SIG hex: {}", hex::encode(sig_v3_empty.to_bytes()));
    println!();
    println!("=== V2 payload (sin platform/deviceFamily) ===");
    println!("TEXT: {payload_v2}");
    println!("SHA256: {}", sha256_hex(payload_v2.as_bytes()));
    println!("SIG hex: {}", hex::encode(sig_v2.to_bytes()));
}
