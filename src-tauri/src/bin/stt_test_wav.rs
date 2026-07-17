//! CLI de prueba: transcribe un archivo WAV usando el modelo STT cargado.
//!
//! Uso:
//!   ./target/release/synapse-cortana stt-test-wav <ruta.wav>
//!   ./target/release/synapse-cortana stt-test-wav /home/macarthur/.config/synapse-cortana/stt-models/sherpa-onnx-streaming-zipformer-en/test_wavs/0.wav

use std::path::PathBuf;

use synapse_cortana::stt::{SttEngine, DEFAULT_STT_MODEL_ID};

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Uso: synapse-cortana stt-test-wav <ruta.wav> [model_id]");
        std::process::exit(1);
    }
    let wav_path = PathBuf::from(&args[1]);
    let model_id = args
        .get(2)
        .cloned()
        .unwrap_or_else(|| DEFAULT_STT_MODEL_ID.to_string());
    if !wav_path.exists() {
        eprintln!("❌ No existe: {}", wav_path.display());
        std::process::exit(1);
    }
    eprintln!("[stt-test] WAV: {}", wav_path.display());
    eprintln!("[stt-test] modelo: {}", model_id);

    let engine = SttEngine::new();
    eprintln!("[stt-test] Cargando modelo {}...", model_id);
    if let Err(e) = engine.set_model(&model_id).await {
        eprintln!("[stt-test] ❌ set_model: {e}");
        std::process::exit(1);
    }
    eprintln!("[stt-test] ✅ Modelo cargado");

    // Leer WAV.
    let mut reader = hound::WavReader::open(&wav_path).expect("abrir WAV");
    let samples: Vec<f32> = reader
        .samples::<i16>()
        .map(|s| s.unwrap_or(0) as f32 / 32768.0)
        .collect();
    let sr = reader.spec().sample_rate;
    eprintln!(
        "[stt-test] {} samples, {} Hz, {} canales",
        samples.len(),
        sr,
        reader.spec().channels
    );

    // Necesitamos mono 16 kHz.
    let mut mono = samples;
    if reader.spec().channels > 1 {
        // Promediar canales.
        mono = mono
            .chunks(reader.spec().channels as usize)
            .map(|c| c.iter().sum::<f32>() / c.len() as f32)
            .collect();
    }
    let samples_16k = if sr == 16000 {
        mono
    } else {
        // Resampleo lineal simple.
        let ratio = 16000.0 / sr as f32;
        let n_out = (mono.len() as f32 * ratio) as usize;
        let mut out = Vec::with_capacity(n_out);
        for i in 0..n_out {
            let src_idx = i as f32 / ratio;
            let i0 = src_idx.floor() as usize;
            let i1 = (i0 + 1).min(mono.len() - 1);
            let frac = src_idx - i0 as f32;
            out.push(mono[i0] * (1.0 - frac) + mono[i1] * frac);
        }
        out
    };
    eprintln!("[stt-test] samples a 16k: {}", samples_16k.len());

    // Crear recognizer.
    let handle = engine
        .handle()
        .await
        .expect("engine.handle: modelo no cargado");
    eprintln!("[stt-test] motor = {:?}", handle.engine_kind);

    match handle.engine_kind {
        synapse_cortana::stt::SttEngineKind::StreamingZipformer => {
            let recognizer = handle
                .online_recognizer_clone()
                .await
                .expect("online_recognizer_clone");
            let stream = recognizer.create_stream();
            // Alimentar en chunks de 1600 samples (100 ms @ 16 kHz).
            let chunk_size = 1600;
            let mut last_partial = String::new();
            let mut total_chunks = 0;
            for chunk in samples_16k.chunks(chunk_size) {
                stream.accept_waveform(16000, chunk);
                total_chunks += 1;
                // Decodificar mientras haya frames listos.
                while recognizer.is_ready(&stream) {
                    recognizer.decode(&stream);
                }
                if let Some(result) = recognizer.get_result(&stream) {
                    let text = result.text.trim();
                    if !text.is_empty() && text != last_partial {
                        eprintln!("[stt-test] parcial #{}: '{}'", total_chunks, text);
                        last_partial = text.to_string();
                    }
                }
            }
            stream.accept_waveform(16000, &[]);
            while recognizer.is_ready(&stream) {
                recognizer.decode(&stream);
            }
            if let Some(result) = recognizer.get_result(&stream) {
                let text = result.text.trim();
                if !text.is_empty() {
                    eprintln!("[stt-test] FINAL: '{}'", text);
                }
            }
            if last_partial.is_empty() {
                eprintln!(
                    "[stt-test] ⚠️ No se transcribió nada (total {} chunks)",
                    total_chunks
                );
                std::process::exit(2);
            }
        }
        synapse_cortana::stt::SttEngineKind::OfflineWhisper => {
            let recognizer = handle
                .offline_recognizer_clone()
                .await
                .expect("offline_recognizer_clone");
            let stream = recognizer.create_stream();
            stream.accept_waveform(16000, &samples_16k);
            recognizer.decode(&stream);
            if let Some(result) = stream.get_result() {
                let text = result.text.trim();
                eprintln!("[stt-test] Whisper FINAL: '{}'", text);
                if text.is_empty() {
                    std::process::exit(2);
                }
            } else {
                eprintln!("[stt-test] Whisper: sin resultado");
                std::process::exit(2);
            }
        }
    }
}
