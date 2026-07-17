# SynapseCortana — Manual de Usuario

Manual completo de la interfaz gráfica de SynapseCortana.

## Ventanas

SynapseCortana abre **dos ventanas** al arrancar:

### Ventana del Chat (principal)

Ventana con bordes, título "Synapse Cortana - Chat", redimensionable. Contiene dos pestañas:

#### Pestaña ⚙️ Configuración

| Opción | Qué hace | Valores |
|--------|---------|--------|
| **URL del Gateway** | Dirección HTTP del gateway OpenClaw | `http://127.0.0.1:18789` (local) o la IP/puerto del túnel SSH |
| **Token** | Token de autenticación del operador | Cadena hexadecimal de ~40 caracteres (de `~/.openclaw/openclaw.json` en el servidor) |
| **Botón "Probar"** | Verifica si el gateway responde (HTTP `/health`) | ✅ accesible / ❌ no accesible |
| **Botón "Conectar"** | Inicia el handshake WebSocket con OpenClaw | Muestra "✅ Conectado al Gateway via WebSocket protocolo v4" o error |
| **Voz TTS** | Selector de voz para síntesis de voz | `es_AR-daniela-high` (mujer argentina, recomendada), `es_ES-mls_9972-low` (mujer castellana, ligera), `es_ES-davefx-medium` (varón), `es_ES-sharvard-medium` (varón), `es_MX-ald-medium` (varón mexicano) |
| **Botón "🔊 Probar voz"** | Sintetiza una frase de prueba con la voz seleccionada | Reproduce audio por los altavoces |
| **Checkbox "Reproducir respuestas de Cortana con TTS"** | Si activo, las respuestas del LLM se sintetizan y reproducen automáticamente | Activado (recomendado) |
| **Sesión** | Selector de sessionKey del gateway | Lista las sesiones disponibles del gateway (ej. `agent:main:main`, `agent:main:telegram:direct:...`) |
| **Botón "🔄"** | Refresca la lista de sesiones | Lee `sessions.list` del gateway |
| **Campo "o pega un sessionKey"** | Introducir un sessionKey manualmente | Para sesiones no listadas por el gateway |
| **Botón "Usar"** | Resuelve y aplica el sessionKey personalizado | Valida con `sessions.resolve` del gateway |
| **Micrófono** | Selector de dispositivo de entrada de audio | Lista los micrófonos del sistema (ej. "default", "USB Mic", etc.) |
| **Modelo STT** | Selector de modelo de reconocimiento de voz | `Whisper medium` (máxima calidad, recomendado), `Whisper base` (más rápido) |
| **Checkbox "Enviar automáticamente al terminar de dictar"** | Si activo, el texto dictado se envía al LLM automáticamente sin pulsar Enter | Útil para interacción 100% por voz |
| **Silencio para fin de respuesta (ms)** | Tiempo de silencio para considerar que el LLM terminó de responder | 500–10000 ms (default 3000). Más bajo = respuesta más rápida. Más alto = más robusto para LLMs lentos |
| **Timeout global (ms)** | Tiempo máximo de espera para la respuesta del LLM | 5000–300000 ms (default 120000 = 2 min) |
| **Botón "🗑️ Vaciar caché TTS"** | Borra todos los WAV cacheados en disco | Libera espacio (~30 KB por segundo de audio) |
| **Indicador de caché** | Muestra "N entradas · X.XX MB" | Cantidad y tamaño del caché TTS actual |
| **Botón "🗑️ Restablecer todo"** | Borra todos los settings y vuelve a defaults | Pierde URL, token, voz, sesión, etc. |

#### Pestaña 💬 Chat

| Elemento | Función |
|---------|--------|
| **Área de mensajes** | Muestra los mensajes: usuario (derecha), Cortana (izquierda), sistema (centro) |
| **Botón 🔊** (en cada mensaje de Cortana) | Re-reproduce el audio TTS de ese mensaje |
| **Indicador "..."** | Aparece cuando el LLM está procesando la respuesta |
| **Campo de texto** | Escribir mensaje |
| **Botón 🎙️** | Inicia/detiene dictado por voz desde el chat |
| **Botón ➤ Enviar** | Envía el mensaje al LLM |
| **Indicador "🔊 TTS listo (22050 Hz)"** | Estado del motor TTS |

### Ventana del Avatar (secundaria)

Ventana sin bordes, transparente, sin barra de tareas. Muestra el modelo 3D de Cortana.

#### Elementos visuales

| Elemento | Descripción |
|---------|-------------|
| **Modelo 3D** | Cortana de cuerpo entero, de frente, con materiales originales |
| **Spinner de carga** | Aparece al iniciar mientras carga el modelo .glb |
| **Indicador de estado** | Texto en la parte inferior: "En espera" (gris), "Escuchando" (amarillo), "Pensando" (azul), "Hablando" (verde) |

#### Controles del mouse

| Acción | Cómo | Resultado |
|--------|------|-----------|
| **Dictar** | Click izquierdo (1 click, sin mover el mouse) | Inicia el dictado por voz. Vuelve a hacer click para detener. Solo el primer click de una ráfaga dispara la acción. |
| **Cerrar aplicación** | 3 clicks izquierdos rápidos (en menos de 600ms) | Cierra el avatar + el chat + termina el proceso. No requiere Ctrl+C. |
| **Mostrar/ocultar chat** | Click derecho | Alterna la visibilidad de la ventana del chat. Si el chat estaba cerrado con (X), se reabre. |
| **Girar modelo** | Rueda del mouse (scroll up/down) | Rota el modelo 3D horizontalmente. Permite ver Cortana desde cualquier ángulo. |
| **Mover ventana** | Click izquierdo + arrastrar más de 10px | Mueve la ventana del avatar a cualquier posición de la pantalla. Usar el gestor de ventanas nativo del SO. |

#### Comportamiento del avatar

| Estado | Cuándo ocurre | Movimiento del modelo |
|--------|--------------|---------------------|
| **En espera** (gris) | Sin actividad | Respiración suave, balanceo lateral muy sutil, oscilación Y de ~5°, head tilt ocasional |
| **Escuchando** (amarillo) | Dictado por voz activo | Quieta, atenta, leve inclinación hacia adelante, micro-vibración |
| **Pensando** (azul) | LLM procesando respuesta | Más erguida, mirando hacia arriba, oscilación Y de ~9° |
| **Hablando** (verde) | TTS reproduciendo audio | Sway más marcado, respiración más rápida, gestos laterales ocasionales |

---

## Flujo de uso por voz (100% voz)

Para interactuar con Cortana completamente por voz:

1. **Configurar** (una sola vez):
   - URL del gateway + Token + Voz TTS + Modelo STT + Micrófono
   - Activar checkbox "Enviar automáticamente al terminar de dictar"
   - Conectar

2. **Hablar con Cortana**:
   - Click izquierdo en el avatar → el indicador cambia a "Escuchando" (amarillo)
   - Habla claramente hacia el micrófono
   - Click izquierdo nuevamente → el indicador cambia a "En espera" (gris)
   - Whisper transcribe tu voz (~3-5 segundos)
   - Si autoSend está activo: el mensaje se envía automáticamente al LLM
   - El indicador cambia a "Pensando" (azul) mientras el LLM procesa
   - El indicador cambia a "Hablando" (verde) mientras Cortana responde con voz
   - Cuando termina, vuelve a "En espera" (gris)

3. **Repetir** cuantas veces quieras.

> **Consejo**: espera a que el indicador vuelva a "En espera" (gris) antes de hacer click para dictar de nuevo. Si dictas mientras Cortana está hablando, el micrófono puede captar el audio del TTS.

---

## Flujo de uso por chat (texto)

1. Escribir mensaje en el campo de texto
2. Pulsar **Enter** (o click en ➤ Enviar)
3. Aparece el indicador "..." mientras el LLM procesa
4. La respuesta de Cortana aparece en el chat
5. Si TTS está activo, el audio se reproduce automáticamente
6. Click en 🔊 para re-reproducir cualquier mensaje anterior

---

## Conexión a gateway remoto (túnel SSH)

Si OpenClaw está en otra máquina:

```bash
# Terminal 1: túnel SSH
ssh -L 18789:127.0.0.1:18789 usuario@IP_DEL_SERVIDOR -N

# Terminal 2: ejecutar SynapseCortana
./"Synapse Cortana_0.1.0_amd64.AppImage"
```

En la aplicación:
- URL del gateway: `http://127.0.0.1:18789`
- Token: el token del operador de OpenClaw

---

## Archivos en disco

SynapseCortana guarda todo en:

```
~/.config/synapse-cortana/
├── settings.json          # Configuración (URL, token, voz, modelo, timeouts)
├── device.key              # Identidad Ed25519 del dispositivo (PEM, permisos 0600)
├── voices/                 # Modelos TTS extraídos del bundle
│   └── es_AR-daniela-high/
├── stt-models/             # Modelos STT extraídos del bundle
│   └── sherpa-onnx-whisper-medium/
├── stt-models/             # Modelos STT extraídos del bundle
│   └── sherpa-onnx-whisper-medium/
├── tts-cache/              # Caché de audio WAV (hash SHA-256 del texto)
└── gstreamer-plugins/     # Plugins de GStreamer extraídos del bundle
```

Para **resetear** la aplicación a estado inicial: borrar `~/.config/synapse-cortana/` y reiniciar.

---

## Solución de problemas

| Problema | Causa probable | Solución |
|----------|---------------|---------|
| "❌ Error de conexión" | Gateway no responde o token incorrecto | Verificar túnel SSH activo, verificar token en `~/.openclaw/openclaw.json` |
| "⏳ Descargando voz..." | La voz no está en el bundle o el bundle no se encontró | Verificar que el AppImage incluye `resources/voices/`. Si se ejecuta desde el binario suelto, descargar la voz la primera vez (requiere internet) |
| "GStreamer element appsink not found" | Plugins de GStreamer no están en el sistema | En AppImage: se extraen automáticamente. En binario suelto: `sudo apt install gstreamer1.0-plugins-base gstreamer1.0-plugins-good` |
| El dictado transcribe mal | Ruido de fondo, micrófono de baja calidad, o el usuario habla demasiado bajo | Hablar claro y directo al micrófono, en un ambiente silencioso |
| El audio del TTS no se reproduce | GStreamer sin plugins o dispositivo de audio no configurado | Verificar que los altavoces funcionan, instalar `gstreamer1.0-plugins-base` |
| La app se cierra sola | Triple-click accidental | Evitar 3 clicks rápidos en el avatar. Un solo click es suficiente para dictar |
| El avatar no aparece | La ventana del avatar se cerró o está detrás de otras ventanas | Reiniciar la aplicación |
| Mensajes truncados | `silence_timeout_ms` muy bajo para el LLM usado | Aumentar a 5000ms en Configuración → "Silencio para fin de respuesta" |
| Logs muy verbosos | RUST_LOG en nivel debug/trace | Usar `RUST_LOG=info` o `RUST_LOG=warn` para menos detalle |

---

## Modo CLI (sin interfaz gráfica)

Útil para testing en servidores sin display, SSH, o CI:

```bash
# Test de handshake con el gateway:
./synapse-cortana --cli-test-handshake \
    --url http://127.0.0.1:18789 \
    --token d86bbd15f647a311ee96322cc579546705023a63813fa20c

# Test de TTS (sintetiza voz y guarda WAV):
./synapse-cortana --cli-test-speak \
    --voice es_AR-daniela-high \
    --text "Hola, soy Cortana. La fase dos del proyecto Synapse ya está hablando." \
    --out /tmp/cortana-test.wav

# Reproducir el WAV generado:
aplay /tmp/cortana-test.wav

# Verificar el WAV:
file /tmp/cortana-test.wav
# → RIFF (little-endian) data, WAVE audio, Microsoft PCM, 16 bit, mono 22050 Hz

# Logs verbosos para depuración:
RUST_LOG=debug ./synapse-cortana
RUST_LOG=synapse_cortana=trace ./synapse-cortana
RUST_LOG=warn ./synapse-cortana    # solo warnings y errores
```

Códigos de salida del CLI:
- `0`: éxito
- `1`: error (handshake fallido, voz desconocida, etc.)
- `2`: transcripción vacía (STT no reconoció nada)