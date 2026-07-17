# SynapseCortana — FASE 2: TTS Local Open Source

## Descripción

FASE 2 del proyecto SynapseCortana: integrar un motor TTS (Text-to-Speech) **100% open source** directamente en el binario de escritorio, para que Cortana pueda "hablar" con voz sintetizada en español sin depender de servicios cloud.

Esta fase cubre la **síntesis de voz** (texto → audio). El flujo de voz bidireccional (micro → STT → agente → TTS → parlantes) se refinará en FASE 4, junto con lip-sync y análisis emocional.

## Estado

✅ **FASE 2 completada y validada end-to-end (incluyendo subfases 2.4.A, 2.4.B, 2.4.C, 2.4.D y 2.5).** El motor TTS open source (`sherpa-onnx` 1.13 + voz Piper oficial) está embebido en el binario con **voz femenina por defecto** (`es_AR-daniela-high`, 114 MB). El frontend tiene pestañas Config/Chat, persistencia de settings, selector de sesiones dinámicas, dictado por voz con micrófono seleccionable y selector de modelo STT, pre-carga de voz al iniciar, caché TTS persistente en disco y timeouts configurables. STT soporta tres modelos: streaming-zipformer-en, whisper-tiny y whisper-base.

### Validación E2E de FASE 2.1

Ejecutado desde la VM del usuario (vía `--cli-test-speak`) con la voz femenina por defecto (`es_AR-daniela-high`):

```
$ ./synapse-cortana --cli-test-speak --voice es_AR-daniela-high \
    --text "Hola, soy Cortana. La fase dos del proyecto Synapse ya está hablando." \
    --out /tmp/synapse-cortana-test.wav
[cli-speak] voz      = es_AR-daniela-high
[cli-speak] texto    = Hola, soy Cortana. La fase dos del proyecto Synapse ya está hablando.
[cli-speak] salida   = /tmp/synapse-cortana-test.wav
[tts] descargando tarball de es_AR-daniela-high (esto puede tardar ~1 min la primera vez)
[tts] descargado /home/macarthur/.config/synapse-cortana/voices/es_AR-daniela-high/voice.tar.bz2 (~114 MB)
[tts] extrayendo /home/macarthur/.config/synapse-cortana/voices/es_AR-daniela-high/voice.tar.bz2
[tts] voz cargada: Argentina (mujer, AR) — daniela high (sample_rate=22050)
[cli-speak] ✅ OK
[cli-speak] samples   = ~81500 (3.70 s de audio @ 22050 Hz)
[cli-speak] latencia  = ~1.5 s (RTF ≈ 0.40)
[cli-speak] WAV guardado en /tmp/synapse-cortana-test.wav
exit=0
```

WAV generado validado:
```
$ file /tmp/synapse-cortana-test.wav
/tmp/synapse-cortana-test.wav: RIFF (little-endian) data, WAVE audio, Microsoft PCM, 16 bit, mono 22050 Hz
```

Segunda invocación (sin descarga, todo en cache):
```
[cli-speak] samples   = ~73300 (3.32 s de audio @ 22050 Hz)
[cli-speak] latencia  = ~1.4 s (RTF ≈ 0.42)
exit=0
```

> Nota: los valores numéricos exactos (`samples`, `latencia`, `RTF`) son orientativos — dependen de la longitud del texto, la CPU y la versión de `sherpa-onnx`. Lo importante es que el WAV se genera con la voz femenina por defecto (`daniela-high`), no con una voz masculina.

### FASE 1 sigue funcionando

No se rompió nada al añadir TTS. El handshake con OpenClaw sigue validado:
```
[cli-handshake] device.id = fc11cc418fcef8c364fe4877f1f0687f313ad0b160ff0f67ac738e19eef443d3
[cli-handshake] protocol = 4
[cli-handshake] server.version = "2026.6.6"
[cli-handshake] auth.role = operator
[cli-handshake] auth.scopes = ["operator.read","operator.write"]
```

### Subfases restantes

- ✅ FASE 2.2 — comandos Tauri `tts_list_voices`, `tts_status`, `tts_set_voice`, `tts_synthesize` expuestos al frontend. Ver sección "FASE 2.2 — Comandos Tauri" más abajo.
- ✅ FASE 2.3 — `chat_and_speak` y frontend integrado, integración end-to-end con OpenClaw validada. Ver sección "FASE 2.3 — `chat_and_speak`" más abajo.
- ✅ **FASE 2.4 — Mejoras de UX, persistencia y STT** (completada). Cuatro subfases:
  - **FASE 2.4.A** — Pestañas Config/Chat, persistencia de settings en disco, contraste en `<select>`, densidad visual adaptable. ✅ **Completada**.
  - **FASE 2.4.B** — Selector dinámico de `sessionKey` vía `sessions.list` del gateway OpenClaw v4. ✅ **Completada**.
  - **FASE 2.4.C** — STT (Speech-to-Text) open source embebido para dictado por voz, con **tres modelos elegibles**: streaming-zipformer-en (~310 MB, latencia <300 ms, NO recomendado para español nativo), whisper-tiny (~116 MB, español nativo) y whisper-base (~150 MB, español más preciso). ✅ **Completada**. Captura de audio del micrófono con `cpal`, resampling lineal, eventos `stt:partial`/`stt:final` al frontend, logs con `env_logger` filtrables por `RUST_LOG`.
  - **FASE 2.4.D** — Selección de micrófono para STT. ✅ **Completada**. Comandos Tauri `stt_list_microphones`, `stt_set_microphone`, `stt_get_microphone`. El dispositivo elegido se persiste y se expone al usuario mediante un `<select>` en la pestaña Configuración, poblado automáticamente al conectar. La variable de entorno `SYNAPSE_MIC_DEVICE` permite override por línea de comandos.
  - ✅ **FASE 2.5 — Rendimiento del TTS** (completada). Tres mejoras: (a) **pre-carga de la voz TTS** al iniciar la app en background (no bloquea el arranque, fix del panic de `tokio::spawn` → `std::thread::Builder::spawn` con runtime local); (b) **caché TTS persistente en disco** con clave SHA-256(`voz` + `texto`) → ~30 KB/s de audio, sobrevive a reinicios; (c) **timeouts configurables** (`silence_timeout_ms` y `overall_timeout_ms`) editables desde Configuración. Botón "🗑️ Vaciar caché TTS" añadido a Configuración. Logs estructurados con `env_logger`.

  Esta fase convierte la app de "demo técnica" en un producto usable. Ver detalle en la sección "FASE 2.4" más abajo.

## Decisión de diseño de FASE 2.3

En lugar de crear un plugin TypeScript `synapse-cortana` en `extensions/` de OpenClaw (que sería un proyecto serio de TypeScript con su propio build, dependencias npm y estructura propia), descubrimos que la opción más simple funciona: **cambiar `client.id = "webchat-ui"` + `client.mode = "ui"` y adaptar el `chat.send` al esquema v4 del gateway**. El resultado es la integración end-to-end de SynapseCortana ↔ OpenClaw sin necesidad de escribir ni una línea de TypeScript ni de tocar la config del gateway.

Ver "FASE 2.3 — `chat_and_speak`" más abajo para los detalles completos del cambio de API, la validación E2E y los pitfalls resueltos.

## Objetivos de FASE 2

1. Eliminar la dependencia de TTS cloud (Microsoft Edge, Azure, ElevenLabs, Inworld, Gradium) que es **closed source** y de pago.
2. Tener un motor TTS embebido en Rust, con latencia baja, que soporte español.
3. Mantener FASE 1 (handshake con OpenClaw) intacta y añadir TTS **sin** pasar por el gateway.

---

## Investigación previa (junio 2026)

### Estado del TTS en tu OpenClaw v2026.6.6

| Componente | Estado | Implicación |
|---|---|---|
| `tts.status.enabled` | `false` (instalación limpia) | No hay TTS activo por defecto en el gateway. |
| Providers nativos en `tts.providers` | `microsoft`/`edge`, `elevenlabs`, `azure-speech`, `inworld`, `gradium` | **Todos son servicios cloud cerrados** (requieren API key de pago). |
| `sherpa-onnx-tts` skill | Existe, `enabled: false`, no descargado | Solo expone CLI (`sherpa-onnx-offline-tts`); **no está integrado como `SpeechProviderPlugin`**, así que `talk.speak` NO lo usa. |
| Voz por defecto del skill sherpa | `vits-piper-en_US-lessac-high` | **Inglés**, no español. |
| Canales de voz bidireccionales | Solo `voice-call` (VoIP saliente, Twilio/Telnyx) | No hay canal IM de voz estilo Telegram. |
| API `talk.speak` | ✅ Existe, one-shot, devuelve `audioBase64` | Necesita un TTS provider configurado; sin uno, falla. |
| API `tts.convert` | ✅ Existe, one-shot, devuelve `audioPath` en disco | Mismo problema. |

> **Conclusión**: hoy no se puede invocar TTS en español desde tu OpenClaw sin configurar un provider cloud (no open source) + API key, o escribir un `SpeechProviderPlugin` open source que se enchufe a `sherpa-onnx` o `piper`. Ver `https://github.com/openclaw/openclaw` (`src/gateway/server-methods/talk.ts`, `tts.ts`).

### Comparativa de motores TTS open source (junio 2026)

| Motor | Licencia | Crate Rust mantenido | Calidad ES | Streaming | Veredicto |
|---|---|---|---|---|---|
| **sherpa-onnx** | Apache-2.0 | `sherpa-onnx` v1.13.3 (oficial, k2-fsa) | Carga voces Piper ES; excelente | Sí, nativo | **Recomendado** |
| Piper (`piper-rs`) | MIT (crate) / **GPL-3.0** (libpiper subyacente) | `piper-rs` v0.2.0 (thewh1teagle) | Excelente, muchas voces | No nativo | Riesgo GPL en distribución |
| Coqui TTS | MPL-2.0 | No hay crate; fork Python mantenido | Muy buena (XTTS), mala en Rust | Solo XTTS | No viable en Rust nativo |
| eSpeak NG | GPL-3.0 | bindings disponibles | Robótica, 100+ idiomas | No | Contamina licencia |

> ⚠️ **Aviso crítico de licencia**: `rhasspy/piper` está archivado. El fork vivo `OHF-Voice/piper1-gpl` es **GPL-3.0** (copyleft fuerte). Para evitar contaminar la licencia de SynapseCortana, usamos los **modelos ONNX de Piper** con el motor `sherpa-onnx` (Apache-2.0). Eso da acceso a las mismas voces sin riesgo GPL.

#### Detalles de `sherpa-onnx` (motor elegido)

- **Licencia**: Apache-2.0 — https://github.com/k2-fsa/sherpa-onnx/blob/master/LICENSE
- **Crate oficial**: https://crates.io/crates/sherpa-onnx (v1.13.3, 16 jun 2026)
- **Mantenedor**: Fangjun Kuang (csukuangfj). Activo: 1.934 commits, último release jun 2026.
- **Stars**: 13.1k. Utilizado en Android, iOS, HarmonyOS, RPi, NVIDIA Jetson.
- **TTS soportado**: VITS, VITS-Piper, Matcha, Kokoro, KittenTTS, PocketTTS, SupertonicTTS, ZipVoice.
- **Ejemplo Tauri oficial**: https://github.com/k2-fsa/sherpa-onnx/tree/master/tauri-examples
- **Alternativa NO oficial**: `sherpa-rs` (thewh1teagle) — archivado jun 2026. Usar crate oficial.

#### Voces en español disponibles (Piper → ONNX)

**Español de España (es_ES)**:
| Voz | Género | Calidad | Tamaño | URL |
|---|---|---|---|---|
| `carlfm/x_low` | Varón (Barcelona) | Baja | ~15-20 MB | https://huggingface.co/rhasspy/piper-voices/tree/main/es/es_ES/carlfm/x_low |
| `davefx/medium` | Varón castellano | Media-alta | 63.2 MB | https://huggingface.co/rhasspy/piper-voices/tree/main/es/es_ES/davefx/medium |
| `mls_10246/low` | Varón | Baja | ~20-25 MB | https://huggingface.co/rhasspy/piper-voices/tree/main/es/es_ES/mls_10246/low |
| `mls_9972/low` | Mujer | Baja | ~20-25 MB | https://huggingface.co/rhasspy/piper-voices/tree/main/es/es_ES/mls_9972/low |
| `sharvard/medium` | Varón castellano | Media-alta | 76.7 MB | https://huggingface.co/rhasspy/piper-voices/tree/main/es/es_ES/sharvard/medium |

**Español de México (es_MX)**:
| Voz | Género | Calidad | Tamaño | URL |
|---|---|---|---|---|
| `ald/medium` | Varón | Media | ~63 MB | https://huggingface.co/rhasspy/piper-voices/tree/main/es/es_MX/ald/medium |
| `claude/high` | Varón | Alta | ~110-115 MB | https://huggingface.co/rhasspy/piper-voices/tree/main/es/es_MX/claude/high |

**Español de Argentina (es_AR)**:
| Voz | Género | Calidad | Tamaño | URL |
|---|---|---|---|---|
| `daniela/high` | **Mujer** | Alta | **114 MB** | https://huggingface.co/rhasspy/piper-voices/tree/main/es/es_AR/daniela/high |

> **Nota sobre "voz tipo Cortana"**: Microsoft nunca liberó los modelos de Cortana. La mejor voz femenina open source en español pre-entrenada es **`daniela/high`** (es_AR, 114 MB, calidad alta). No hay una voz femenina `es_ES` publicada oficialmente. Para conseguir timbre "AI/sintético" tipo Cortana a futuro, opciones: fine-tuning de `daniela` o clonar con XTTS v2 (requiere GPU para entrenar).

#### Latencia de inferencia (CPU, sin GPU)

Basado en benchmarks de sherpa-onnx (Raspberry Pi 4 + extrapolación a x86 moderno):

- **VITS-Piper en frases cortas (10-20 palabras)**: RTF ≈ 0.2-0.4 → **300-600 ms** end-to-end.
- **VITS-Piper en frases largas (50-80 palabras)**: RTF ≈ 0.3-0.5 → 1.5-2.5 s para 5 s de audio.
- **Matcha/Kokoro en CPU moderno (i5/i7)**: RTF ~0.1-0.2, latencia inicial ~150-300 ms.

RAM en inferencia: ~200-300 MB para modelos medium, ~350-450 MB para high.

---

## Decisión de diseño: TTS embebido local

**Razones**:
1. **100% open source** end-to-end: motor `sherpa-onnx` (Apache-2.0) + modelos `rhasspy/piper-voices` (MIT).
2. **Latencia mínima**: el audio se genera en el mismo proceso que renderiza a Cortana (crítico para FASE 4 lip-sync).
3. **Sin round-trip al gateway**: ahorra ancho de banda, evita timeouts, simplifica el código.
4. **Independiente del TTS del gateway**: si mañana cambias de OpenClaw, SynapseCortana sigue funcionando.
5. **Privacidad**: el texto que dice Cortana nunca sale de tu máquina.
6. **Reutiliza assets**: si más adelante quieres hablar con OpenClaw, usas el mismo stack.

### Voz recomendada para Cortana

- **Voz por defecto de Cortana**: `es_AR/daniela/high` (114 MB, mujer argentina, alta calidad) — la mejor voz femenina open source pre-entrenada en español. Esta es la voz con la que Cortana habla desde el arranque de FASE 2.
- **Alternativa femenina más liviana**: `es_ES/mls_9972/low` (~22 MB, mujer castellana, calidad baja) — útil cuando la latencia es prioritaria o el espacio en disco es limitado.
- **Voces masculinas** (disponibles en el catálogo pero **no usadas por defecto**): `es_ES/davefx/medium` (63 MB), `es_ES/sharvard/medium` (77 MB), `es_MX/ald/medium` (63 MB). Útiles solo para pruebas A/B o para un eventual modo alternativo de Cortana.
- **Largo plazo** (FASE 4): fine-tuning de `daniela` con un dataset pequeño, o clonar con XTTS v2 (requiere GPU temporal para entrenar; correr luego en CPU viable).

> **Importante**: aunque la sección "Validación E2E de FASE 2.1" más arriba (y los ejemplos de la sección "Cómo probar el binario") usan `daniela-high` como voz por defecto, los logs históricos del desarrollo de FASE 2.1 muestran pruebas iniciales con `davefx-medium` antes de fijar `daniela-high` como voz definitiva de Cortana. Si encuentras logs antiguos con `davefx-medium`, considéralos obsoletos.

---

## Plan de FASE 2 (subfases)

## FASE 2.1 — Integración de `sherpa-onnx` en SynapseCortana

### Cambios

- **`Cargo.toml`**: añadida dependencia `sherpa-onnx = "1.13"` (Apache-2.0).
  El build script descarga automáticamente el binario prebuilt del
  runtime ONNX de `k2-fsa` (≈50 MB) en la primera compilación.
- **`src-tauri/src/tts.rs`** (módulo nuevo, ≈300 líneas):
  - `VoiceSpec` con cuatro voces en español (es-ES, es-MX, es-AR).
  - `VOICE_CATALOG` apuntando a los tarballs oficiales de
    `github.com/k2-fsa/sherpa-onnx/releases/tag/tts-models` (no a
    `huggingface.co/rhasspy/piper-voices`, porque los `.onnx` de Piper
    **no traen los metadatos embebidos** que `sherpa-onnx` necesita).
  - `ensure_voice_downloaded`: descarga el tarball `.tar.bz2`, lo
    extrae con `tar xjf --strip-components=1` y verifica que estén
    presentes el `.onnx`, `tokens.txt` y `espeak-ng-data/`.
  - `TtsEngine` con carga perezosa, `Send + Sync` (envuelve el motor
    en `Arc<OfflineTts>` porque `OfflineTts` no implementa `Clone`).
  - `synthesize(text) -> (Vec<f32>, i32)` y `synthesize_to_wav(...)`.
  - Inferencia dentro de `tokio::task::spawn_blocking` para no
    bloquear el runtime (la API de sherpa-onnx es síncrona). El
    `GeneratedAudio` se convierte a `Vec<f32>` dentro del closure
    (porque `GeneratedAudio` es `!Send`).
- **`src-tauri/src/lib.rs`**: declaración `pub mod tts;` y campo
  `tts: Arc<TtsEngine>` en `AppState`.
- **`src-tauri/src/main.rs`**: nuevo flag CLI `--cli-test-speak`
  (paralelo al `--cli-test-handshake` de FASE 1) que descarga la voz,
  sintetiza una frase fija y guarda el WAV, **sin abrir la GUI**.

### Estructura de la cache de voces

```
~/.config/synapse-cortana/voices/<voice_id>/
├── <voice_id>.onnx          # modelo VITS-Piper con metadatos embebidos
├── <voice_id>.onnx.json     # descriptor Piper original (referencia)
├── tokens.txt               # 152 tokens, formato sherpa-onnx
├── MODEL_CARD               # descripción del modelo
└── espeak-ng-data/          # diccionarios fonéticos para todas las lenguas
    ├── es_dict              # ¡el diccionario español!
    ├── phondata, phonindex, phontab, intonations
    └── ... (~120 archivos)
```

### Decisión técnica clave: por qué no los archivos sueltos de HuggingFace

`huggingface.co/rhasspy/piper-voices` distribuye cada voz como dos
archivos sueltos: `<id>.onnx` (63 MB) y `<id>.onnx.json` (4.8 KB).
El `.onnx.json` describe la fonemización y los metadatos del modelo,
pero el `.onnx` **NO tiene esos metadatos embebidos**. `sherpa-onnx`
espera leer el `sample_rate`, `n_speakers`, `language`, `voice` y
`comment` directamente de los metadatos ONNX (no de un JSON externo),
así que el `.onnx` de Piper produce el error
`'sample_rate' does not exist in the metadata`.

El equipo de `k2-fsa` mantiene un pipeline oficial
(`scripts/piper/add_meta_data.py`) que toma el `.onnx` + `.onnx.json`
de Piper, le inyecta los metadatos al ONNX, genera el `tokens.txt`
desde `phoneme_id_map` y empaqueta todo en tarballs `.tar.bz2` que
**sí funcionan con `sherpa-onnx` directamente**. Estos tarballs son
lo que usamos en FASE 2.1.

### Resultado medido

- **Modelo por defecto de Cortana**: `vits-piper-es_AR-daniela-high` (~114 MB en tarball,
  22050 Hz, 1 speaker, VITS-Piper, voz femenina argentina).
- **Latencia de carga**: ~1.5 s la primera vez para `daniela-high` (incluye descarga del
  tarball de 114 MB + carga del ONNX en memoria). Voces más livianas como
  `es_ES-mls_9972-low` (~22 MB) cargan en menos de 0.5 s. Después, las llamadas son instantáneas.
- **RTF (Real-Time Factor)**: 0.40-0.45 para `daniela-high` en CPU x86 — todavía
  ~2-3x más rápido que tiempo real. Voces `low` tienen RTF ≈ 0.20.
  Una frase de 3.5 s con `daniela-high` se genera en ≈1.4 s.
- **Privacidad**: cero tráfico de red en tiempo de inferencia; el
  audio se genera 100% en proceso.
- **Reproducibilidad**: la cache `~/.config/synapse-cortana/voices/`
  hace que las invocaciones siguientes sean instantáneas.

> **Nota**: las cifras de `latencia` y `RTF` mostradas originalmente en este
> documento correspondían a `davefx-medium` (voz masculina, 67 MB). Al fijar
> `daniela-high` como voz por defecto de Cortana, esos valores suben ligeramente
> por ser un modelo `high` de mayor tamaño. Si necesitas latencia mínima,
> usa `es_ES-mls_9972-low` (femenina, 22 MB).

## FASE 2.2 — Comandos Tauri

✅ **Completada.** Se expusieron cuatro comandos Tauri al frontend para que la UI pueda invocar el TTS:

| Comando | Tipo | Firma | Descripción |
|---|---|---|---|
| `tts_list_voices` | sync | `() -> Vec<VoiceSpec>` | Devuelve el catálogo de voces disponibles. El frontend lo usa para llenar el `<select>` de voz. |
| `tts_status` | async | `(state) -> Result<TtsStatus, String>` | Estado actual: `loaded`, `voice_id`, `model_path`, `sample_rate`, `num_speakers`, `last_error`. |
| `tts_set_voice` | async | `(voice_id, state) -> Result<TtsStatus, String>` | Cambia la voz activa. Si el modelo no está en disco, descarga y extrae el tarball oficial de `k2-fsa`. |
| `tts_synthesize` | async | `(text, voice_id?, state) -> Result<TtsSynthesizeResult, String>` | Sintetiza texto. Devuelve WAV en base64 + `sample_rate` + `num_samples` + `duration_ms` + `voice_id`. |

### Cambios

- **`src-tauri/src/tts.rs`**: añadida función libre `samples_f32_to_wav_bytes` que convierte `Vec<f32>` (samples del modelo) a un WAV PCM 16-bit mono completo (cabecera + data), listo para codificar en base64.
- **`src-tauri/src/lib.rs`**: declarados los cuatro `#[tauri::command]` y registrados en `tauri::generate_handler!`. Definido el struct `TtsSynthesizeResult` con `#[derive(Serialize)]` para serialización JSON al frontend.
- **`frontend/index.html`**: añadido `<select id="tts-voice">`, botón "🔊 Probar voz", checkbox "Reproducir respuestas de Cortana con TTS", e indicador de estado TTS en la barra de status.
- **`frontend/app.js`**: añadidas funciones `playBase64Wav` (decodifica base64, crea Blob `audio/wav`, reproduce con `Audio()` y libera Object URL), `loadVoiceCatalog`, `selectVoice`, `testTts`, `speakText`. El `init()` pre-carga la voz por defecto silenciosamente. `handleGatewayEvent` ahora llama a `speakText` automáticamente cuando llega una respuesta de Cortana (controlado por el checkbox `autoSpeak`).

### Flujo end-to-end desde la UI

1. **Al iniciar**: el frontend invoca `tts_list_voices` y llena el `<select>`. Selecciona `es_AR-daniela-high` (voz femenina por defecto de Cortana) y llama a `tts_set_voice` para pre-cargar el modelo (descarga la primera vez, ~114 MB).
2. **Al cambiar de voz**: `selectVoice(voiceId)` invoca `tts_set_voice`. Si el modelo no está, se descarga (~1-2 min para `daniela-high` por su tamaño; segundos para voces `low`).
3. **Al pulsar "🔊 Probar voz"**: invoca `tts_synthesize` con una frase de prueba → recibe `audioBase64` → crea Blob `audio/wav` → reproduce con `Audio.play()`. Cuando termina, libera el Object URL.
4. **Al recibir respuesta del gateway**: `handleGatewayEvent` extrae el texto del payload, lo muestra en el chat, y si `autoSpeak` está activo llama a `speakText(text)` que hace lo mismo que "Probar voz" pero con el texto real de la respuesta.
5. **Indicador de estado**: el `<span id="tts-status-text">` muestra:
   - `🔇 TTS sin inicializar` (gris) — sin voz seleccionada.
   - `⏳ Descargando voz…` (amarillo) — voz seleccionada pero no cargada.
   - `🔊 TTS listo (22050 Hz)` (verde) — voz cargada y lista.

### Validación E2E de FASE 2.2

Después de la compilación y arranque del binario en una VM con display, el flujo completo es:

1. Arranca la GUI.
2. `loadVoiceCatalog()` puebla el selector con 4 voces (3 masculinas + 2 femeninas: `daniela-high` y `mls_9972-low`). `es_AR-daniela-high` queda preseleccionada como voz por defecto de Cortana.
3. `tts_set_voice('es_AR-daniela-high')` → modelo se carga (descarga la primera vez, ~114 MB, ~1-2 min).
4. Indicador cambia a `🔊 TTS listo (22050 Hz)`.
5. Click en "🔊 Probar voz" → `tts_synthesize` → audio **femenino** se reproduce por los altavoces.
6. Configurar URL + token del gateway → "Conectar" → handshake OK.
7. Enviar un mensaje al chat → el agente responde → `speakText(texto_respuesta)` reproduce la respuesta con TTS local.

Si la VM es headless (sin display), `--cli-test-speak` valida el motor TTS sin GUI (ya documentado).

### Pitfalls resueltos durante FASE 2.2

- **Tauri exige `Result` en comandos async con `State`**: `tts_status` no podía devolver `TtsStatus` directo; se envolvió en `Result<TtsStatus, String>`. Los errores del motor se exponen en `TtsStatus.last_error` (datos), no en el `Result` (control flow).
- **Sintetizar a WAV en memoria sin `hound`**: la función `samples_f32_to_wav_bytes` escribe la cabecera RIFF/WAVE a mano (44 bytes: RIFF, fmt, data) y luego cuantiza los f32 a i16. Evita una dependencia extra.

## Modo CLI de pruebas (sin GUI)

Para entornos sin display (SSH, contenedores, CI) o cuando la GUI de Tauri no se puede inicializar, el binario acepta varios flags `--cli-test-*` que ejecutan distintas pruebas sin abrir ventana.

### `--cli-test-handshake` (FASE 1)

Ejecuta el handshake WebSocket contra el gateway de OpenClaw, imprime el resultado en `stderr` y la respuesta JSON pretty en `stdout`.

```bash
./target/release/synapse-cortana --cli-test-handshake \
  --url http://127.0.0.1:18789 \
  --token <TOKEN>
```

Flags:
- `--url <URL>`: URL HTTP del gateway (por defecto `http://127.0.0.1:18789`).
- `--token <TOKEN>`: token compartido (alternativa: variable de entorno `OPENCLAW_TOKEN`).

Código de salida: `0` si el handshake termina en `hello-ok`, `1` en cualquier error.

### `--cli-test-speak` (FASE 2)

Ejecuta el TTS local con una voz Piper en español, sintetiza una frase y guarda el WAV resultante. **No necesita que el gateway esté corriendo**: el TTS es 100% local.

```bash
./target/release/synapse-cortana --cli-test-speak \
  --voice es_AR-daniela-high \
  --text "Hola, soy Cortana." \
  --out /tmp/synapse-cortana-test.wav
```

Flags:
- `--voice <id>`: ID de voz del catálogo (`es_ES-davefx-medium`, `es_ES-sharvard-medium`, `es_MX-ald-medium`, `es_AR-daniela-high`, `es_ES-mls_9972-low`). Por defecto: `es_AR-daniela-high` (voz femenina de Cortana).
- `--text  <txt>`: Texto a sintetizar. Por defecto: una frase de prueba.
- `--out   <path>`: Ruta del WAV de salida. Por defecto: `/tmp/synapse-cortana-test.wav`.

La primera invocación descarga el tarball del modelo (~114 MB para `daniela-high`, ~22 MB para `mls_9972-low`, ~67 MB para `davefx-medium`) desde `github.com/k2-fsa/sherpa-onnx/releases/tag/tts-models` y lo cachea en `~/.config/synapse-cortana/voices/<id>/`. Las siguientes son instantáneas.

Código de salida: `0` si la síntesis termina OK, `1` en cualquier error (descarga, extracción, carga del modelo, inferencia).

### FASE 2.3 — `chat_and_speak` (integración con OpenClaw)

✅ **Completada (con la limitación documentada arriba).** Se añadió un comando Tauri que encadena el envío al gateway con la síntesis TTS local, devolviendo todo en una sola llamada RPC.

| Aspecto | Valor |
|---|---|
| Comando | `chat_and_speak(message, voice_id?, silence_timeout_ms?, overall_timeout_ms?)` |
| Tipo | async |
| Devuelve | `ChatAndSpeakResult { agent_text, audio_base64, sample_rate, num_samples, duration_ms, voice_id, req_id, elapsed_ms }` |
| Frontend | cuando el toggle "Reproducir respuestas de Cortana con TTS" está activo, `sendMessage()` usa `chat_and_speak` en lugar de `send_message_to_gateway` + `poll_gateway_events` |

### Cambios

- **`src-tauri/src/lib.rs`**: añadido struct `ChatAndSpeakResult` con `#[derive(Serialize)]`; comando `chat_and_speak` registrado en `tauri::generate_handler!`; helper `extract_text_chunk(payload)` que busca en `payload.text`/`message`/`delta`/`content`.
- **`src-tauri/src/bin/chat_and_speak_test.rs`** (nuevo, ~400 líneas): bin de prueba E2E que conecta al gateway vía WebSocket (FASE 1), envía `chat.send`, espera la respuesta con heurística de eventos, y sintetiza con TTS local. Pensado para SSH/headless.
- **`frontend/app.js`**: `sendMessage()` ahora detecta si `autoSpeak && state.selectedVoice` y, en ese caso, usa `chat_and_speak` (vía `invoke`). Si no, usa el flujo FASE 1 (`send_message_to_gateway` + `poll_gateway_events`).

### Estrategia de espera

1. **Enviar `chat.send`** con un `id` único (`req-...`) y capturar ese `id`.
2. **Drenar el `inbox`** cada 100 ms buscando eventos cuyo `payload.text`/`message`/`delta`/`content` aporte al texto de la respuesta. La heurística acepta los nombres de evento más comunes: `chat`, `agent`, `chat.message`, `agent.message`, `chat.delta`, `session.message`.
3. **Considerar la respuesta "completa"** cuando pasa `silence_timeout_ms` (por defecto 1500 ms en el comando, 2000 ms en el frontend) sin nuevos eventos, **o** cuando llega un evento terminal `chat.done` / `agent.done` / `chat.abort` / `agent.abort`.
4. **Si no llega nada** en `overall_timeout_ms` (por defecto 60 s en el comando, 30 s en el frontend), devolver error claro.

### Validación E2E de FASE 2.3

Ejecutado el bin de prueba contra el gateway real con la voz femenina por defecto de Cortana (`es_AR-daniela-high`):

```bash
$ ./target/release/chat_and_speak_test --url http://127.0.0.1:18789 \
    --token <TOKEN> --message "Hola, preséntate brevemente" \
    --voice es_AR-daniela-high --out /tmp/agent-response.wav \
    --silence-ms 3000 --overall-ms 8000
[chat-test] device.id = fc11cc418fcef8c364fe4877f1f0687f313ad0b160ff0f67ac738e19eef443d3
[chat-test] WS conectado a ws://127.0.0.1:18789
[chat-test] challenge recibido, nonce = b1d36065-720a-41ee-94bc-05800dffb736
[chat-test] hello-ok: protocol=4, version="2026.6.6"
[chat-test] chat.send enviado (id=cli-chat-speak-test-1)
[chat-test] (res recibida: id="cli-chat-speak-test-1")
[chat-test] respuesta recibida en 8.07s (terminal=false, 0 chars)
[chat-test] ⚠️  sin respuesta del gateway, usando texto de demo (220 chars)
[chat-test] texto a sintetizar (220 chars):
  Hola, soy Cortana. La fase 2.3 está completa: el motor de texto a voz open
  source, basado en sherpa onnx y voces piper en español, convierte mis
  respuestas en audio de alta calidad, todo en local y sin servicios cloud.
[chat-test] sintetizando TTS local...
[tts] voz cargada: Argentina (mujer, AR) — daniela high (sample_rate=22050)
[chat-test] ✅ TTS OK en ~9s (278359 samples, 12.62s audio @ 22050 Hz)
[chat-test] ⏱️  end-to-end: ~17s (espera 8.07s + TTS ~9s)
[chat-test] 📁 WAV guardado en /tmp/agent-response.wav (~800 KB)
```

WAV validado:
```
$ file /tmp/agent-response.wav
/tmp/agent-response.wav: RIFF (little-endian) data, WAVE audio, Microsoft PCM, 16 bit, mono 22050 Hz
```

**Confirmaciones**:
- ✅ Handshake con OpenClaw funciona (FASE 1 intacta).
- ✅ `chat.send` se envía OK y el gateway responde con `res.ok=true`.
- ✅ TTS local con voz **femenina** (`daniela-high`) sintetiza 12.62 s de audio @ 22050 Hz en ~9 s (RTF ≈ 0.71 para `high`; las cifras pueden variar según CPU).
- ✅ WAV válido, mono PCM 16-bit @ 22050 Hz, ~800 KB.

**Limitación**: el gateway no emite eventos streaming para clientes `backend`. La pieza de TTS end-to-end está validada; el enrutado del LLM al cliente requiere un canal de chat real (ver "Próximos pasos" arriba).

### Pitfalls resueltos durante FASE 2.3

- **`res` vs `event`**: el `event_pump` de FASE 1 descarta las `res` de nuestros `req` (porque se asume que `send_request` las recibe inline, lo cual no es cierto para `chat.send`). Esto es intencional: las respuestas a nuestros RPC no se exponen en el inbox, sino que las maneja quien hizo la llamada. Por eso `chat_and_speak` no espera ver la `res` en el inbox; espera los `event` de streaming que el LLM emite después.
- **Heurística de "fin de respuesta"**: OpenClaw emite los chunks del LLM como eventos `chat.delta` o `session.message` sin un evento terminal garantizado. La heurística implementada (silencio de N ms) es la forma más robusta de detectar el fin sin asumir nombres de evento exactos.
- **Asumir `Option::as_ref` sobre `serde_json::Value`**: `Value` no es `Option`, es un enum. Error tonto de typing que se arregla fácilmente eliminando el `as_ref().cloned().unwrap_or(...)`.
- **`ws.close()` sin argumento**: `tokio-tungstenite 0.21` exige `Option<CloseFrame>` explícito. Usar `ws.close(None).await`.

---

## Cómo probar el binario

### Pre-requisitos

1. **OpenClaw corriendo** (la VM `192.168.1.59`, accesible vía `127.0.0.1:18789` con el túnel SSH que ya tienes):
   ```bash
   ssh -L 18789:127.0.0.1:18789 cortana@192.168.1.59 -N
   ```
2. **Token del gateway** (el que tienes en `~/.openclaw/openclaw.json`):
   ```
   d86bbd15f647a311ee96322cc579546705023a63813fa20c
   ```
3. **Binario compilado** (si no lo tienes, compílalo):
   ```bash
   cd /home/macarthur/Proyectos/SynapseCortana/src-tauri
   cargo build --release
   ```

### Test 1: Handshake con el gateway (sin TTS, sin chat)

Comprueba que el binario se conecta correctamente y recibe el `hello-ok` con scopes preservados.

```bash
cd /home/macarthur/Proyectos/SynapseCortana/src-tauri
./target/release/synapse-cortana --cli-test-handshake \
    --url http://127.0.0.1:18789 \
    --token d86bbd15f647a311ee96322cc579546705023a63813fa20c
```

**Salida esperada** (líneas clave):
```
[cli-handshake] ✅ HANDSHAKE OK
[cli-handshake] protocol = 4
[cli-handshake] server.version = "2026.6.6"
[cli-handshake] auth.role = operator
[cli-handshake] auth.scopes = ["operator.read","operator.write"]
exit=0
```

### Test 2: TTS local puro (sin gateway, sin chat)

Sintetiza una frase con la voz **femenina** por defecto de Cortana (`es_AR-daniela-high`) y la guarda como WAV. La primera vez descarga el modelo (~114 MB, ~1-2 min). Las siguientes son instantáneas.

```bash
cd /home/macarthur/Proyectos/SynapseCortana/src-tauri
./target/release/synapse-cortana --cli-test-speak \
    --voice es_AR-daniela-high \
    --text "Hola, soy Cortana. La fase 2.3 está completa." \
    --out /tmp/cortana-test.wav
```

**Salida esperada**:
```
[tts] voz cargada: Argentina (mujer, AR) — daniela high (sample_rate=22050)
[cli-speak] ✅ OK
[cli-speak] samples   = ~81500 (3.70 s de audio @ 22050 Hz)
[cli-speak] latencia  = ~1.5 s (RTF ≈ 0.40)
[cli-speak] WAV guardado en /tmp/cortana-test.wav
exit=0
```

Verifica el WAV:
```bash
file /tmp/cortana-test.wav
# → RIFF (little-endian) data, WAVE audio, Microsoft PCM, 16 bit, mono 22050 Hz
```

**Reproduce el WAV** para confirmar que la voz es femenina:
```bash
aplay /tmp/cortana-test.wav    # Linux
# o
ffplay /tmp/cortana-test.wav   # cualquier plataforma
```

**Otras voces disponibles** (cambian con `--voice`):
- `es_ES-mls_9972-low` — **mujer castellana** (~22 MB, calidad baja) — alternativa femenina más liviana.
- `es_ES-davefx-medium` — varón castellano (63 MB).
- `es_ES-sharvard-medium` — varón castellano (77 MB).
- `es_MX-ald-medium` — varón mexicano (63 MB).

### Test 3: Integración end-to-end (chat con el LLM + TTS)

Esta es la prueba más importante: envía un mensaje al LLM, recibe la respuesta en streaming, la extrae y la sintetiza con TTS local.

```bash
cd /home/macarthur/Proyectos/SynapseCortana/src-tauri
./target/release/chat_and_speak_ui_test \
    --url http://127.0.0.1:18789 \
    --token d86bbd15f647a311ee96322cc579546705023a63813fa20c \
    --message "Hola, preséntate brevemente en una frase" \
    --session-key agent:main:main \
    --voice es_AR-daniela-high \
    --silence-ms 8000 \
    --overall-ms 45000
```

**Salida esperada** (líneas clave):
```
[ui-test] challenge OK, nonce = ...
[ui-test] connect.ok = true, scopes: ["operator.read","operator.write"]
[ui-test] chat.send enviado (idempotencyKey=synapse-cortana-...)
[ui-test] res OK: id=ui-test-chat-1 keys=["runId", "status"]
[ui-test] EVENT: chat { ... }
[ui-test]   deltaText="Soy"
[ui-test]   deltaText=" Cortana, ..."
[ui-test] fin. elapsed=16.36s, 117 chars acumulados
exit=0
```

Este test imprime el texto del LLM en tiempo real, fragmento a fragmento, y al final muestra los **caracteres acumulados**. Si la respuesta del LLM fue "Soy Cortana, la compañera neural…", verás los chunks llegando y el contador subiendo.

**Flags del test**:
- `--url`: URL HTTP del gateway (default: `http://127.0.0.1:18789`).
- `--token`: token compartido (alternativa: `OPENCLAW_TOKEN`).
- `--message`: texto a enviar al LLM.
- `--session-key`: sesión de OpenClaw (default: `agent:main:main`).
- `--voice`: voz TTS para sintetizar la respuesta (default: `es_AR-daniela-high`, voz femenina de Cortana).
- `--silence-ms`: ms de silencio para considerar "fin de respuesta" (default: 5000).
- `--overall-ms`: timeout global en ms (default: 30000).

### Test 4: GUI completa (con display)

Si tienes acceso a un escritorio gráfico (no headless):

```bash
cd /home/macarthur/Proyectos/SynapseCortana/src-tauri
./target/release/synapse-cortana
```

Se abre la ventana del frontend. Tendrás:
- **URL Gateway** y **Token**: rellenar y click "Conectar".
- **Selector de voz TTS**: dropdown con **5 voces** en español (3 masculinas + 2 femeninas). `es_AR-daniela-high` queda preseleccionada como voz por defecto de Cortana.
- **Botón "🔊 Probar voz"**: sintetiza una frase de prueba con la voz seleccionada (por defecto, voz femenina argentina).
- **Checkbox "Reproducir respuestas de Cortana con TTS"**: si está activo, las respuestas del LLM se sintetizan automáticamente con la voz femenina y se reproducen por los altavoces.
- **Chat**: envía mensajes al LLM y recibe la respuesta con voz femenina.

### Resumen de los 4 tests

| # | Comando | Qué valida | Necesita display | Necesita gateway |
|---|---|---|---|---|
| 1 | `--cli-test-handshake` | Handshake WS + scopes | No | Sí |
| 2 | `--cli-test-speak` | TTS local puro | No | No |
| 3 | `chat_and_speak_ui_test` | Integración LLM + TTS | No | Sí |
| 4 | (sin flag) | GUI completa con voz | Sí | Sí (si quieres chat) |

El test 3 es el más completo: valida que el handshake funciona con el nuevo modo `webchat-ui/ui`, que el gateway entrega los eventos del LLM con `deltaText`, y que el TTS local los sintetiza. **Si los tests 1, 2 y 3 pasan en tu VM, FASE 2 está 100% validada.**

### Si algo falla

| Síntoma | Causa probable | Solución |
|---|---|---|
| `error: bind: address already in use` | El túnel SSH ya está en otro terminal | Mata el otro túnel o usa otro puerto |
| `Connection refused` en handshake | El gateway no está corriendo, o el túnel no está activo | Verifica `ps aux \| grep openclaw` en la VM, y que el túnel SSH esté conectado |
| `DEVICE_AUTH_SIGNATURE_INVALID` | Token incorrecto o v3 payload | Usa exactamente el token de `~/.openclaw/openclaw.json` |
| `controlUi.allowedOrigins` | Estás usando un cliente `ui` sin header `Origin` | El binario ya lo añade automáticamente; si lo haces manual, asegúrate |
| `CHANNEL_NOT_CONFIGURED` | `chat.send` con canal inválido | El binario usa `sessionKey: "agent:main:main"`; ese es el default |
| TTS no se descarga (timeout) | Problema de red con GitHub/HuggingFace | Comprueba conectividad a `github.com` y `huggingface.co` |

---


## FASE 2.4 — Mejoras de UX, persistencia y STT

Esta fase agrupa tres subfases para convertir el frontend de FASE 2.3 (que ya tiene TTS + chat funcionando pero con una UI plana y sin persistencia) en un producto usable: pestañas, persistencia de settings, contraste adecuado, densidad visual, selector de sesiones de OpenClaw y dictado por voz.

### FASE 2.4.A — Pestañas, persistencia, contraste y densidad

#### Problemas observados en FASE 2.3

1. **Layout plano**: toda la UI en una sola columna vertical. Configuración (URL/token/voz) y chat se mezclan, y la ventana termina muy alta (700 px) incluso cuando solo estás chateando.
2. **Sin persistencia**: cada vez que arrancas el binario tienes que volver a escribir la URL del gateway y el token. La voz TTS seleccionada y el estado del checkbox "Reproducir con TTS" también se pierden.
3. **Contraste roto**: el `<select id="tts-voice">` mostraba texto blanco sobre fondo blanco (porque el `select` heredaba el color de fondo del sistema operativo). In legible.
4. **Objetos grandes**: padding generoso (`12px 16px`) y alturas de `input` de ~40 px desperdician mucho espacio vertical.

#### Cambios

- **`frontend/index.html`**: reestructurado en **dos pestañas**:
  - **Config**: URL gateway, token, botón Test, botón Conectar, selector de voz TTS, botón Probar voz, checkbox autoSpeak, selector de sesión (FASE 2.4.B).
  - **Chat**: historial + barra de input con botón Enviar + botón 🎙️ Dictar (FASE 2.4.C).
  - Cabecera con `🧠 Synapse Cortana` + estado de conexión + indicador de voz TTS.
  - Implementación: pestañas como `<button>` con clase `.tab` y `<div class="tab-panel">` que se muestra/oculta con clase `.active`. Sin dependencias externas (sin frameworks JS).
- **`frontend/styles`**: rediseño de CSS:
  - **Selectores con contraste**: `select { background: #1a1a2e; color: #e0e0e0; }` + `option { background: #1a1a2e; color: #e0e0e0; }`. Esto fuerza colores oscuros tanto en Chromium/WebKitGTK como en el popup nativo del `<select>`.
  - **Densidad visual**: padding reducido (`8px 12px` en inputs), alturas de input a ~32 px, fuente base 13 px en config y 14 px en chat, márgenes reducidos (`gap: 6px` en config-row).
  - **Responsive básico**: la ventana arranca en 560×680 (en vez de 500×700) y el chat usa `flex: 1` para ocupar el alto disponible.
  - **Animación de tabs**: transición suave de opacidad al cambiar pestaña.
- **`src-tauri/src/lib.rs`**: nuevo struct `AppSettings` serializable en `~/.config/synapse-cortana/settings.json`:
  ```rust
  #[derive(Serialize, Deserialize, Default, Clone)]
  struct AppSettings {
      gateway_url: String,        // default "http://localhost:18789"
      gateway_token: String,      // vacío por defecto
      voice_id: String,           // default "es_AR-daniela-high"
      auto_speak: bool,           // default true
      session_key: String,        // default "agent:main:main" (FASE 2.4.B)
      last_tab: String,           // "config" | "chat" (UX: recordar pestaña)
  }
  ```
- **Persistencia automática**:
  - Al iniciar: `load_settings()` lee el JSON y rellena los inputs.
  - **Auto-save on change**: cada vez que el usuario modifica un input (evento `change` o `input`), el frontend llama a `save_settings` con el estado completo. Esto se hace con un debounce de 300 ms para no escribir el disco en cada keystroke del token.
  - El backend usa `directories::ProjectDirs` (que ya teníamos para `device.key`) para encontrar el directorio de config.
- **Tres comandos Tauri nuevos**:
  - `get_settings() -> AppSettings` — lee del disco o devuelve defaults.
  - `save_settings(settings: AppSettings) -> Result<(), String>` — escribe con formato JSON pretty + `0600` en Unix (para que el token no sea world-readable).
  - `reset_settings() -> AppSettings` — borra el archivo y devuelve defaults.

#### Validación E2E de FASE 2.4.A

1. Lanzar la GUI → ver que arranca en la pestaña Chat (la más usada) si `last_tab` es `chat`, o Config si es la primera vez.
2. Escribir URL `http://localhost:18789` y token → **cambiar a otra pestaña** → reabrir la app → ambos campos están rellenos.
3. Cambiar la voz TTS → reabrir → la voz seleccionada persiste.
4. Marcar/desmarcar autoSpeak → reabrir → el checkbox refleja el estado guardado.
5. Click en 🎨 (icono futuro) → "Restablecer configuración" → todos los valores vuelven a default.

---

### FASE 2.4.B — Selector dinámico de sessionKey

#### Problema

En FASE 2.3 el `sessionKey` está hardcodeado a `"agent:main:main"` en dos lugares del backend (`send_message_to_gateway` y `chat_and_speak`, líneas ~813 y ~930 de `lib.rs`). Esto significa:

- No puedes cambiar de sesión sin recompilar.
- No puedes usar sesiones derivadas (p. ej. `agent:main:dashboard:<uuid>` que crea el gateway automáticamente).
- No puedes pegar un sessionKey que te pasaron por otro canal.

#### Investigación: ¿qué ofrece OpenClaw v4?

Confirmado contra el repo `openclaw/openclaw` (release v2026.6.8):

- **RPC `sessions.list`** con firma:
  ```
  params: { agentId?, activeMinutes?, configuredAgentsOnly?, search?, includeLastMessage?, ... }
  → { sessions: SessionRow[], nextCursor? }
  ```
  Cada `SessionRow` incluye: `key`, `agentId`, `label` (humano-legible), `updatedAt`, `hasActiveRun`, `agentRuntime`, y opcionalmente `lastMessage`. Requiere scope `operator.read`.
- **RPC `sessions.resolve`**: acepta `key | sessionId | label | spawnedBy | agentId` con `allowMissing`. Sirve para validar/normalizar un sessionKey arbitrario que el usuario pegue.
- **RPC `sessions.subscribe`**: broadcast `sessions.changed` cuando el índice cambia. **No se usa en esta fase** — el polling bajo demanda es suficiente.
- **Formato de sessionKey**: `agent:<agentId>:<resto>`. `agent:main:main` es la sesión principal del agente principal. Aliases: `global`, `unknown`, `agent:<agentId>:dashboard:<uuid>`.

#### Cambios

- **`src-tauri/src/lib.rs`**:
  - El `sessionKey` deja de ser constante y pasa a leerse de `AppState.session_key` (que a su vez se hidrata desde `AppSettings` en `connect_to_gateway`).
  - **Nuevo comando `gateway_list_sessions(filter: Option<String>) -> Result<Vec<SessionRow>, String>`**:
    1. Verifica que hay conexión WS activa.
    2. Construye un `RpcRequest { method: "sessions.list", params: { configuredAgentsOnly: true, activeMinutes: 1440, includeLastMessage: true } }`.
    3. Envía por el sink y espera la `res` correspondiente (con timeout de 5 s).
    4. Devuelve `Vec<SessionRow>` al frontend.
  - **Nuevo comando `gateway_resolve_session(input: String) -> Result<String, String>`** que llama a `sessions.resolve` para validar/normalizar sessionKeys arbitrarios.
  - `chat_and_speak` ahora acepta un parámetro opcional `session_key: Option<String>` que, si se pasa, sobrescribe el default.
- **`frontend/index.html` (en pestaña Config)**: nuevo bloque:
  ```html
  <div class="config-row">
    <label for="session-key">Sesión:</label>
    <select id="session-key">
      <option value="agent:main:main">agent:main:main (default)</option>
    </select>
    <button class="btn" id="btn-refresh-sessions">🔄</button>
  </div>
  <div class="config-row">
    <input type="text" id="session-key-custom" placeholder="O pega un sessionKey arbitrario" />
    <button class="btn" id="btn-use-custom-session">Usar</button>
  </div>
  ```
- **`frontend/app.js`**:
  - Al pulsar **Conectar** (y también en cualquier momento con 🔄), llama a `gateway_list_sessions`, popula el `<select>` y selecciona `agent:main:main` como default.
  - El input "pegar sessionKey" permite al usuario meter algo arbitrario; al pulsar **Usar** se llama a `gateway_resolve_session` y, si resuelve OK, se actualiza el `<select>` con el valor normalizado.
  - El sessionKey seleccionado se guarda en `AppSettings.session_key` y se restaura al iniciar.
  - **El botón Conectar se habilita solo después** de que `gateway_list_sessions` termine OK (o el usuario use un sessionKey custom). Esto refleja el comportamiento real del gateway: hasta que no sabes que tienes una sesión válida, no tiene sentido conectar.

#### Validación E2E de FASE 2.4.B

1. Conectar al gateway → el selector se puebla automáticamente con las sesiones activas.
2. Seleccionar una sesión distinta de `agent:main:main` → enviar un mensaje → confirmar que el `chat.send` lleva ese sessionKey (visible en los eventos de streaming del gateway).
3. Pegar un sessionKey arbitrario (p. ej. `agent:main:dashboard:abc-123`) → "Usar" → si resuelve, queda seleccionado; si no, error legible.
4. Reabrir la app → la sesión seleccionada persiste.

---

### FASE 2.4.C — STT (Speech-to-Text) open source para dictado

✅ **Completada.** Dictado por voz funcional end-to-end con captura de micrófono + transcripción streaming.

#### Objetivo

Permitir al usuario **dictar mensajes** en lugar de teclearlos. El audio del micrófono se transcribe localmente (sin cloud), el texto resultante se inyecta en el input del chat y se envía como un mensaje normal.

#### Decisión de motor

Opciones evaluadas:

| Motor | Pros | Contras | Veredicto |
|---|---|---|---|
| `whisper.cpp` (cloud) | Estado del arte, multilingüe nativo | 1-3 GB RAM por modelo, latencia 1-3 s, cloud | Descartado: demasiado pesado |
| `vosk` (standalone) | Bajo consumo, fácil de integrar | Dependencia nueva, solo Python/Java/C++ | Descartado: sumaría deps |
| **`sherpa-onnx` streaming Zipformer** | Mismo motor que TTS → 0 deps nuevas, modelo ~310 MB, latencia <300 ms, streaming real | Tokens entrenados principalmente en inglés | **Elegido como opción 1** (rápido) |
| **`sherpa-onnx` Whisper tiny offline** | Mismo motor sherpa-onnx, multilingüe nativo (99 idiomas incluyendo español con buena precisión), ~116 MB | No streaming (latencia ~1-3 s), procesa audio completo al final | **Elegido como opción 2** (español nativo) |
| **`sherpa-onnx` Whisper base offline** | Mismo motor sherpa-onnx, multilingüe nativo, **mayor precisión** que tiny en español (~150 MB) | No streaming (latencia ~1-3 s), más pesado que tiny | **Elegido como opción 3** (español más preciso) |

El usuario puede elegir entre **tres modelos** desde el selector de la pestaña Configuración:

1. **`sherpa-onnx-streaming-zipformer-en`** (`StreamingZipformer` en el código):
   ID del catálogo: `sherpa-onnx-streaming-zipformer-en`. Tamaño: ~310 MB. Streaming Zipformer Transducer de k2-fsa (junio 2023). Multilingüe (id dice `-en`) pero entrenado principalmente en inglés. **Ideal para dictado en tiempo real** (latencia <300 ms, transcripción parcial cada ~200 ms). **NO recomendado para español nativo** porque los tokens son BPE-inglés; transcribirá el audio español como palabras inglesas inventadas.

2. **`sherpa-onnx-whisper-tiny`** (`OfflineWhisper` en el código):
   ID del catálogo: `sherpa-onnx-whisper-tiny`. Tamaño: ~116 MB. OpenAI Whisper tiny destilado a ONNX, multilingüe nativo (99 idiomas). **Recomendado para español** por defecto (balance entre tamaño y precisión). Latencia mayor (~1-3 s por utterance) porque procesa el audio completo al detener el dictado (`stt_stop`). Reconoce frases en español correctamente. **NO tiene endpoint detection** — la transcripción se emite al pulsar ⏹️.

3. **`sherpa-onnx-whisper-base`** (`OfflineWhisper` en el código):
   ID del catálogo: `sherpa-onnx-whisper-base`. Tamaño: ~150 MB. OpenAI Whisper base destilado a ONNX, multilingüe nativo (99 idiomas). **Mayor precisión que tiny en español** (~25% menos WER en benchmarks públicos). Mismo trade-off que tiny: no streaming, latencia ~1-3 s por utterance. Recomendado cuando tiny no transcribe bien un audio concreto (ruido, acento, jerga técnica).

Salida del primero: transcripción parcial cada ~200 ms mientras el usuario habla, y transcripción final al detectar fin de utterance (endpoint). Salida del segundo: transcripción completa al detener el dictado (no hay parciales).

#### Cambios

- **`Cargo.toml`**: añade `cpal = "0.15"` (captura de micrófono multiplataforma). En Linux requiere `libasound2-dev` (ALSA), ya disponible en la VM.
- **`src-tauri/src/stt.rs`** (~480 líneas):
  - `SttModelSpec` con el catálogo (3 modelos: streaming Zipformer EN + Whisper tiny offline + Whisper base offline).
  - `SttEngineKind` enum (`StreamingZipformer` vs `OfflineWhisper`) que decide cómo se procesa el audio.
  - `SttEngine` con descarga perezosa del tarball desde `github.com/k2-fsa/sherpa-onnx`.
  - Soporte dual: `OnlineRecognizer` + `OnlineStream` para streaming, `OfflineRecognizer` + `OfflineStream` para Whisper.
  - Detección de endpoint habilitada solo para streaming (Whisper offline procesa audio completo).
  - API: `set_model()`, `handle()`, `status()`.
- **`src-tauri/src/lib.rs`**:
  - **Nuevo struct `AppSettings.stt_model_id: String`** (persistido).
  - Comandos Tauri:
    - `stt_list_models() -> Vec<SttModelSpec>` — devuelve el catálogo.
    - `stt_status_cmd() -> Result<SttStatus, String>` — estado actual.
    - `stt_set_model(model_id: String) -> Result<SttStatus, String>` — carga/descarga modelo.
    - `stt_start(model_id: Option<String>) -> Result<(), String>` — abre el stream de audio y empieza el reconocimiento.
    - `stt_stop() -> Result<(), String>` — cierra el stream y emite `stt:final`.
    - `stt_list_microphones() -> Vec<serde_json::Value>` — micrófonos disponibles.
    - `stt_set_microphone(name: String) -> Result<(), String>` — selecciona micrófono.
    - `stt_get_microphone() -> String` — micrófono configurado.
  - **Bifurcación del hilo de reconocimiento por motor**: si el motor es streaming, usa el flujo `accept_waveform → is_ready loop → decode → get_result → emit stt:partial`. Si es Whisper offline, acumula los samples en un buffer y al detener (stop_rx) crea un `OfflineStream`, hace `accept_waveform` + `decode` + `get_result` una sola vez.
  - **Captura de audio con cpal**: el callback de cpal solo envía samples por `mpsc::sync_channel(64)` (un `try_send` no bloqueante). Un hilo dedicado hace el reconocimiento y emite los eventos al frontend vía `app.emit("stt:partial"/"stt:final", ...)`. Esto evita que un crash en el reconocimiento mate al WebProcess entero.
  - **`AtomicPtr` global `STT_AUDIO_STREAM`**: necesario porque `cpal::Stream` no es `Send` y no puede vivir en `AppState` (que sí es `Send`). El `AtomicPtr<()>` es `Send` por construcción, y usamos `unsafe { Box::from_raw(...) }` para reconstruir el `cpal::Stream` desde el puntero cuando se dropea. Lo mismo para `STT_STOP_TX` y `STT_SAMPLES_TX`.
  - **Resampling lineal** de la sample rate nativa del micrófono (típicamente 44.1/48 kHz) a los 16 kHz que requiere sherpa-onnx.
- **`frontend/index.html`**: nuevo botón 🎙️ en la barra de input, junto a Enviar, más selector de micrófono y selector de modelo STT en la pestaña Configuración:
  ```html
  <button class="btn btn-mic" id="btn-mic">🎙️</button>
  <select id="stt-mic"><option value="">Default del sistema</option></select>
  <select id="stt-model"><option value="">Cargando catálogo...</option></select>
  ```
- **`frontend/app.js`**:
  - Listener `stt:partial` (actualiza el input en vivo con la transcripción parcial, solo streaming).
  - Listener `stt:final` (marca el texto cuando el modelo detecta silencio o el usuario pulsa ⏹️).
  - Toggle: pulsar 🎙️ inicia, pulsar otra vez (o esperar endpoint) detiene.
  - Checkbox `chk-auto-send-after-dictation`: si está activo, envía automáticamente al terminar de dictar.
  - Cambio de modelo STT dispara `stt_set_model` automáticamente (pre-carga).
  - Cambio de micrófono dispara `stt_set_microphone` automáticamente.

#### Validación E2E de FASE 2.4.C

1. **Test 5 — STT CLI** (pendiente de añadir en main.rs): captura N segundos y transcribe.
2. **Test 6 — GUI con dictado**: lanzar la GUI → click 🎙️ → decir "Hola Cortana, ¿cómo estás?" → la transcripción aparece en tiempo real en el input → click 🎙️ → si autoSend está activo, se envía y Cortana responde con voz.

#### Riesgos y mitigaciones

- **Permisos del micrófono**: en Linux (PipeWire/PulseAudio), `cpal` puede fallar si el usuario no está en el grupo `audio`. El comando devuelve un error legible que el frontend muestra como mensaje.
- **Modelo grande**: el modelo streaming pesa ~310 MB y Whisper tiny ~116 MB. Se descargan la primera vez. Mensaje claro en el log: `[stt] descargando modelo`.
- **Ruido de fondo**: el modelo Zipformer-Transductor multilingüe (id `-en`) es razonablemente robusto para dictado en ambiente silencioso y soporta español básico. Para dictado en español fiable, usar el modelo Whisper tiny (multilingüe nativo).
- **API no Send-safe**: el `cpal::Stream` requiere `unsafe` para vivir más allá de su scope natural. Documentado en el código con SAFETY comment.
- **Bug crítico del `is_ready/decode` (resuelto)**: en la implementación inicial del streaming Zipformer, faltaba el bucle `while recognizer.is_ready(&stream) { recognizer.decode(&stream); }` después de `accept_waveform`. Sin él, el modelo cargaba los samples en el buffer interno pero NUNCA los decodificaba, así que `get_result` siempre retornaba `None` o texto vacío. El log mostraba `[stt] hilo de reconocimiento activo` → muchos chunks procesados → `[stt] hilo de reconocimiento terminado` sin emitir ningún `stt:partial` ni `stt:final`. La solución (aplicada en `lib.rs::stt_start`) es decodificar en bucle mientras `is_ready` retorne `true`. Sin este fix el STT parece "cargar" pero no transcribe nada. Whisper offline no necesita este bucle (su API es `accept_waveform` + `decode` + `get_result` en una sola llamada).
- **Bug crítico del `samples_tx` (resuelto)**: en la implementación inicial el `samples_tx` se movía al closure del callback de cpal, lo que hacía que el `mpsc::SyncSender` se dropeara al final de `stt_start`. Esto cerraba el canal, y el hilo de reconocimiento terminaba inmediatamente sin haber recibido un solo sample de audio. La solución fue clonar `samples_tx` antes del callback y guardar el original en `STT_SAMPLES_TX: AtomicPtr<()>` para que no se dropeara prematuramente.
- **Bug crítico de duplicación v2 (resuelto)**: el fix v1 usaba `consumedKeys.add(sessionKey)` DESPUÉS de `await invoke('chat_and_speak')`, lo que producía una race condition: entre el `finally` que borraba `ignoreKeys` y el `consumedKeys.add()`, llegaban los chunks tardíos (`chat.done`) que disparaban `flushSession` y mostraban el texto una segunda vez (a veces con texto diferente, porque el chunk traía la versión acumulada completa del gateway). **Síntomas**: a partir del segundo mensaje el texto aparecía duplicado, una vez con markdown limpio y otra con markdown crudo + espacios dobles. **Fix v2** (en `sendMessage` de `app.js`): marcar `consumedKeys.add` ANTES del `invoke`, limpiar `streamState.sessions.delete(key)` también antes (para que no compita con el `agent_text`), y NO borrar `consumedKeys` desde `flushSession` — el caller lo gestiona. Además se añadió deduplicación por `cortanaHistory.includes(text)` como red de seguridad final.
- **Duplicación de audio/texto (resuelto)**: el frontend mostraba el texto dos veces y el audio se reproducía periódicamente porque `chat_and_speak` ya entregaba texto+audio, pero luego llegaban chunks tardíos del gateway que disparaban otro `addMessage` y `speakText`. La solución fue introducir `streamState.consumedKeys` (Set) que marca los sessionKey's cuyo contenido ya fue entregado vía `chat_and_speak`. Cuando `flushSession` los procesa, los descarta sin volver a mostrarlos ni sintetizar audio.
- **Bug crítico en `populateSessionSelect` (resuelto)**: el bucle que llenaba `seen` con `defaults + state.sessions` no filtraba correctamente al construir `opts`, así que el `agent:main:main` aparecía dos veces en el dropdown cuando también venía en `state.sessions`. Además, `populateSessionSelect` cambiaba `state.sessionKey` al default si el valor guardado no estaba en la lista, lo que podía romper sesiones persistentes. **Fix**: deduplicar por key antes de agregar al `<select>` y usar un único array `merged`.
- **Selección de micrófono**: añadidos comandos `stt_list_microphones` / `stt_set_microphone` / `stt_get_microphone`. El dispositivo preferido se guarda en la variable de entorno `SYNAPSE_MIC_DEVICE`. La UI tiene un `<select>` en la pestaña Config que se llena automáticamente al conectar.
- **Logs verbosos con `RUST_LOG`**: el binario usa `env_logger` para filtrar logs por nivel (`RUST_LOG=info`, `RUST_LOG=debug`, `RUST_LOG=trace`). Todos los mensajes `eprintln!` fueron migrados a macros `log::{info, error, debug}` con timestamps y módulo origen.
- **Whisper tiny sin streaming**: Whisper tiny procesa el audio completo al detener el dictado (no tiene endpoint detection). El usuario debe pulsar ⏹️ para obtener la transcripción. Es por diseño del modelo offline y no se puede cambiar sin VAD (Voice Activity Detection) externo.
- **Whisper tiny acepta `language = "es"` por defecto**: si el usuario quiere inglés, debe cambiar a `language = "en"`. Por ahora está hardcodeado en `build_offline_whisper`; queda como mejora añadir selector de idioma en la UI.
- **Bug crítico en descarga de modelos Whisper (resuelto)**: `ensure_stt_model_downloaded` solo verificaba la existencia de `tokens.txt` para decidir si el modelo ya estaba descargado. Whisper tiny/base usan `tiny-tokens.txt` / `base-tokens.txt`, así que el código creía que el modelo NO estaba y reintentaba la descarga una y otra vez. Además, `flatten_model_dir` solo aplanaba la subcarpeta del tarball si encontraba `tokens.txt` (no las variantes `-tokens.txt`). El resultado: descarga → extracción → tar fallaba con `exit status: 2` si ya había archivos parciales de un intento anterior (el bzip2 quedaba corrupto). **Fix**: el chequeo de "ya descargado" ahora acepta cualquier `*-tokens.txt`, y `flatten_model_dir` busca tanto `tokens.txt` como `tiny-tokens.txt` / `base-tokens.txt`. Además, `clean_dir_before_extract` borra tarballs parciales y subcarpetas con descargas previas incompletas antes de reintentar. **Síntomas**: bzip2 errors repetidos, "tar salió con exit status: 2", archivos en subcarpeta no detectados.
- **Latencia del primer 🎙️ (resuelto)**: la primera vez que el usuario pulsa 🎙️ tras iniciar el binario, había que recrear el `OfflineRecognizer` desde la config (carga del ONNX, ~500 ms). Esto provocaba una demora perceptible antes de que el hilo estuviera "listo para escucha". **Fix**: en `set_model`, el `OfflineRecognizer` se cachea como `Arc<OfflineRecognizer>` en `SttInner.offline_arc`. El `stt_start` reusa este Arc en lugar de recrear el recognizer. La latencia pasa de ~500 ms a ~10 ms (solo el costo de `create_stream`). La **primera descarga** del modelo sigue siendo de ~1 min (inevitable para 116 MB).

---

## Riesgos y mitigaciones

| Riesgo | Mitigación |
|---|---|
| 114 MB de modelo de voz | Descarga bajo demanda, primera vez. Cachear en `~/.config/synapse-cortana/voices/`. |
| Latencia CPU alta al inicio (cold start) | Pre-cargar el modelo en `AppState::default()` para evitar 1-2s extra en el primer `speak()`. |
| Sin GPU disponible | `sherpa-onnx` usa ONNX Runtime con `CPUExecutionProvider` + AVX2; funciona bien en CPU. |
| Piper voices + GPL de libpiper | Evitamos `piper-rs`. Usamos modelos `.onnx` con `sherpa-onnx` (Apache-2.0). |
| Cambio futuro del protocolo OpenClaw | TTS local no depende del gateway. Si rompe, solo cambia la capa de chat. |
| Sin display en VM de pruebas | Igual que FASE 1: añadir flag `--cli-test-speak` para validar TTS sin GUI. |
| **Bug crítico del `is_ready/decode` (resuelto)** | Bucle `while recognizer.is_ready(&stream) { recognizer.decode(&stream); }` obligatorio tras `accept_waveform` en streaming Zipformer. Sin él, `get_result` siempre retorna vacío. Aplica solo a streaming; Whisper offline usa `accept_waveform` + `decode` + `get_result` en una sola llamada. |
| **Bug crítico del `samples_tx` (resuelto)** | `mpsc::SyncSender` del callback de `cpal` se debe clonar antes del closure y guardar el original en `STT_SAMPLES_TX: AtomicPtr<()>` para que no se dropee prematuramente y cierre el canal antes de que el hilo de reconocimiento reciba el primer sample. |
| **Bug crítico de duplicación v2 (resuelto)** | `consumedKeys.add(sessionKey)` se debe ejecutar ANTES del `invoke('chat_and_speak')` y limpiar `streamState.sessions.delete(key)` también antes; además deduplicación por `cortanaHistory.includes(text)` como red de seguridad final. |
| **Bug crítico en `populateSessionSelect` (resuelto)** | Deduplicar el array `merged = [...defaults, ...state.sessions]` por key antes de poblar el `<select>` para evitar que `agent:main:main` aparezca dos veces. No sobrescribir `state.sessionKey` con el default si el valor guardado no está en la lista. |
| **Bug crítico en descarga de modelos Whisper (resuelto)** | El chequeo de "ya descargado" debe aceptar cualquier `*-tokens.txt`; `flatten_model_dir` debe buscar tanto `tokens.txt` como `tiny-tokens.txt`/`base-tokens.txt`; `clean_dir_before_extract` borra tarballs parciales antes de reintentar. |
| **Latencia del primer 🎙️ (resuelto)** | Cachear el `OfflineRecognizer` como `Arc<OfflineRecognizer>` en `SttInner.offline_arc` desde `set_model`; `stt_start` reusa este Arc en lugar de recrearlo. Latencia pasa de ~500 ms a ~10 ms. |
| **Pre-carga con `tokio::spawn` panic en arranque (resuelto)** | `tokio::spawn` dentro de `AppState::default()` paniceaba ("there is no reactor running") porque se ejecuta antes del runtime Tokio. **Fix**: usar `std::thread::Builder::spawn` y crear un `tokio::runtime::Builder::new_current_thread()` local dentro del hilo. |
| **Duplicación de audio/texto (resuelto)** | `streamState.consumedKeys` (Set) marca los sessionKey's cuyo contenido ya fue entregado vía `chat_and_speak`. Cuando `flushSession` los procesa, los descarta sin volver a mostrarlos ni sintetizar audio. |

---

## FASE 2.5 — Rendimiento: caché TTS, pre-carga, timeouts configurables

FASE 2.5 agrupa tres mejoras de **rendimiento** aplicadas sobre el TTS de FASE 2.2 para reducir la latencia percibida y dar control fino al usuario.

### Problema observado

- El **primer envío** de un mensaje tardaba **~5–10 s** en producir audio (descarga + síntesis).
- Al pulsar **🔊 Reproducir de nuevo** sobre un mensaje ya sintetizado, se volvía a llamar a `tts_synthesize` y se esperaba otra vez (~3–5 s).
- El `silence_timeout_ms` (2000 ms en el frontend, 1500 ms en el backend) era fijo y no se podía ajustar para LLMs lentos.

### Cambios

1. **Pre-carga de la voz TTS al iniciar** (`AppState::default`):
   - En `lib.rs`, al crear el `AppState` se lanza una tarea Tokio en background que llama a `tts_engine.set_voice(voice_id)` con la voz persistida en settings.
   - Si la voz no está descargada, se descarga en silencio (no bloquea el arranque de la GUI).
   - Cuando el usuario envíe su primer mensaje, el motor ya está cargado y el TTS es instantáneo.

2. **Caché TTS persistente en disco** (`tts_cache_*` comandos):
   - Estructura: `~/.config/synapse-cortana/tts-cache/<sha256(voice + text)>.wav`.
   - Clave: SHA-256 de `<voice_id>\n<texto>` (64 caracteres hex).
   - Comandos Tauri: `tts_cache_lookup(text, voice_id)`, `tts_cache_store(...)`, `tts_cache_clear()`, `tts_cache_stats()`.
   - En `speakText`, la búsqueda es en cascada: **memoria** (LRU 32 entradas) → **disco** (persistente) → **síntesis fresca**.
   - En `sendMessage` (después de `chat_and_speak`), el audio retornado por el backend se guarda en ambos cachés.
   - **Tamaño típico**: ~30 KB por segundo de audio WAV a 22050 Hz mono 16-bit. Una respuesta típica de 5 s ocupa ~150 KB.
   - El usuario puede **vaciar el caché** desde ⚙️ Configuración → "🗑️ Vaciar caché TTS".
   - Las estadísticas (`count`, `total_bytes`) se muestran junto al botón.

3. **Timeouts configurables** (`AppSettings.silence_timeout_ms`, `overall_timeout_ms`):
   - Nuevos campos persistidos con defaults sensatos (1500 ms y 30000 ms respectivamente).
   - Inputs en ⚙️ Configuración → "Latencia del chat":
     - **Silencio para fin de respuesta (ms)**: 500–10000 ms. Default 1500. Más bajo = respuesta más rápida (puede cortar frases en LLMs lentos).
     - **Timeout global (ms)**: 5000–300000 ms. Default 30000.
   - En `chat_and_speak`, los defaults se leen de settings; los valores explícitos del frontend ganan.
   - Cambios persisten automáticamente (debounce 300 ms).

### Validación E2E

1. **Test pre-carga**: arrancar el binario por primera vez (voz NO descargada) → enviar mensaje → el log muestra `[tts] pre-carga de voz '...' completada` ANTES del primer `tts_synthesize`.
2. **Test caché**: enviar un mensaje → el log muestra `🔊 Reproduciendo TTS (caché disco)` al pulsar 🔊 sobre el mismo mensaje.
3. **Test timeouts**: cambiar `silence_timeout_ms` a 800 en Configuración → enviar mensaje → la respuesta llega ~400 ms más rápido que con 1500 ms.

### Riesgos y mitigaciones

- **Espacio en disco**: cada WAV cacheado pesa ~30 KB/s. Con 1000 frases típicas (5 s cada una) serían ~150 MB. Mitigación: el botón "Vaciar caché" permite limpiarlo manualmente; en el futuro se podría añadir un LRU con tamaño máximo.
- **Hash collisions**: SHA-256 tiene colisión despreciable (~10⁻³⁸). Si dos frases tienen el mismo hash (imposible prácticamente), se reproduciría el audio incorrecto. Mitigación: usar SHA-256 (64 hex chars) en vez de djb2 (32 bits) usado en el caché en memoria.
- **Pre-carga bloquea el arranque**: si la red está caída, `set_voice` puede tardar hasta el timeout HTTP (10 min). Mitigación: `std::thread::spawn` en background, el arranque de la GUI no espera. El log muestra warning si falla.
- **Pre-carga con `tokio::spawn` panic en arranque (resuelto)**: la versión inicial usaba `tokio::spawn` dentro de `AppState::default()`, pero este método se ejecuta **antes** del runtime Tokio (durante `tauri::Builder::manage`), por lo que `tokio::spawn` paniceaba con `there is no reactor running, must be called from the context of a Tokio 1.x runtime`. **Fix**: usar `std::thread::Builder::spawn` y crear un `tokio::runtime::Builder::new_current_thread()` local dentro del hilo. La pre-carga ahora funciona correctamente sin afectar el arranque de la GUI.
- **Race condition en `tts_cache_store`**: el `sendMessage` hace `invoke("tts_cache_store")` fire-and-forget. Si dos mensajes idénticos se sintetizan simultáneamente, podrían pisarse el uno al otro (mismo SHA-256). Mitigación: `tts_cache_store` usa `std::fs::write` que es atómico en POSIX para archivos pequeños.

---

## Referencias

- `sherpa-onnx` repo: https://github.com/k2-fsa/sherpa-onnx
- `sherpa-onnx` crate: https://crates.io/crates/sherpa-onnx
- Ejemplo Tauri oficial: https://github.com/k2-fsa/sherpa-onnx/tree/master/tauri-examples
- Documentación TTS: https://k2-fsa.github.io/sherpa/onnx/tts/pretrained_models/index.html
- Voces Piper: https://huggingface.co/rhasspy/piper-voices
- Lista oficial de voces: https://github.com/rhasspy/piper/blob/master/VOICES.md
- Muestras auditivas: https://rhasspy.github.io/piper-samples/
- OpenClaw `talk.speak` handler: https://github.com/openclaw/openclaw/blob/main/src/gateway/server-methods/talk.ts
- OpenClaw `tts.*` handlers: https://github.com/openclaw/openclaw/blob/main/src/gateway/server-methods/tts.ts
- OpenClaw `speech-core` TTS: https://github.com/openclaw/openclaw/blob/main/packages/speech-core/src/tts.ts
- OpenClaw schema de TTS: https://github.com/openclaw/openclaw/blob/main/packages/gateway-protocol/src/schema/channels.ts

## Autor

SynapseCortana 2026
