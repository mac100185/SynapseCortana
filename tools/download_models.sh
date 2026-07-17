#!/bin/bash
# Descarga los modelos TTS y STT necesarios para el bundle offline.
# Ejecutar antes de `cargo tauri build`.
#
# Uso:
#   ./tools/download_models.sh
#
# Este script descarga:
#   - Voz TTS es_AR-daniela-high (114 MB) → src-tauri/resources/voices/
#   - Modelo STT Whisper medium int8 (900 MB) → src-tauri/resources/stt-models/
#   - Plugins de GStreamer (24 MB) → src-tauri/resources/gstreamer-plugins.tar

set -e

RESOURCES_DIR="src-tauri/resources"
mkdir -p "$RESOURCES_DIR/voices" "$RESOURCES_DIR/stt-models"

echo "=== Descargando modelos para SynapseCortana ==="
echo ""

# 1. Voz TTS: es_AR-daniela-high
VOICE_DIR="$RESOURCES_DIR/voices/es_AR-daniela-high"
if [ -d "$VOICE_DIR" ] && [ -f "$VOICE_DIR/es_AR-daniela-high.onnx" ]; then
    echo "✅ Voz TTS ya descargada: $VOICE_DIR"
else
    echo "📥 Descargando voz TTS es_AR-daniela-high (114 MB)..."
    TMPDIR=$(mktemp -d)
    wget -q -O "$TMPDIR/voice.tar.bz2" \
        "https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/vits-piper-es_AR-daniela-high.tar.bz2"
    mkdir -p "$VOICE_DIR"
    tar xjf "$TMPDIR/voice.tar.bz2" -C "$VOICE_DIR" --strip-components=1
    rm -rf "$TMPDIR"
    echo "✅ Voz TTS descargada en $VOICE_DIR"
fi

# 2. Modelo STT: Whisper medium (int8, máxima calidad)
STT_DIR="$RESOURCES_DIR/stt-models/sherpa-onnx-whisper-medium"
if [ -d "$STT_DIR" ] && [ -f "$STT_DIR/medium-encoder.int8.onnx" ]; then
    echo "✅ Modelo STT ya descargado: $STT_DIR"
else
    echo "📥 Descargando modelo STT Whisper medium (1.9 GB)..."
    TMPDIR=$(mktemp -d)
    wget -q -O "$TMPDIR/medium.tar.bz2" \
        "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-whisper-medium.tar.bz2"
    mkdir -p "$STT_DIR"
    tar xjf "$TMPDIR/medium.tar.bz2" -C "$TMPDIR"
    # Copiar solo los archivos int8 (más pequeños, mejor calidad/tamaño)
    cp "$TMPDIR/sherpa-onnx-whisper-medium/medium-decoder.int8.onnx" "$STT_DIR/"
    cp "$TMPDIR/sherpa-onnx-whisper-medium/medium-encoder.int8.onnx" "$STT_DIR/"
    cp "$TMPDIR/sherpa-onnx-whisper-medium/medium-tokens.txt" "$STT_DIR/"
    rm -rf "$TMPDIR"
    echo "✅ Modelo STT descargado en $STT_DIR"
fi

# 3. Plugins de GStreamer (para AppImage)
GST_TAR="$RESOURCES_DIR/gstreamer-plugins.tar"
if [ -f "$GST_TAR" ]; then
    echo "✅ Plugins de GStreamer ya empaquetados: $GST_TAR"
else
    echo "📥 Empaquetando plugins de GStreamer..."
    TMPDIR=$(mktemp -d)
    mkdir -p "$TMPDIR/gstreamer-plugins"
    cp /usr/lib/x86_64-linux-gnu/gstreamer-1.0/*.so "$TMPDIR/gstreamer-plugins/" 2>/dev/null || true
    if [ -f /usr/lib/x86_64-linux-gnu/gstreamer1.0/gstreamer-1.0/gst-plugin-scanner ]; then
        cp /usr/lib/x86_64-linux-gnu/gstreamer1.0/gstreamer-1.0/gst-plugin-scanner "$TMPDIR/gstreamer-plugins/"
    fi
    tar cf "$GST_TAR" -C "$TMPDIR" gstreamer-plugins/
    rm -rf "$TMPDIR"
    echo "✅ Plugins de GStreamer empaquetados en $GST_TAR"
fi

echo ""
echo "=== Modelos listos para build ==="
echo "Voz TTS:    $(du -sh $VOICE_DIR | cut -f1)"
echo "Modelo STT: $(du -sh $STT_DIR | cut -f1)"
echo "GStreamer:  $(du -sh $GST_TAR | cut -f1)"
echo ""
echo "Ahora puedes ejecutar: cargo tauri build"
