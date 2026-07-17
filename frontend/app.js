// ============================================
// SYNAPS CORTANA - FASE 2.4
// TTS local + STT local + sesiones dinámicas + persistencia
// ============================================

// Tauri 2 con `withGlobalTauri: true` expone el `invoke` en
// `window.__TAURI__.core.invoke` (no en `window.__TAURI__.invoke` como
// en Tauri 1). Detectamos ambas formas para mantener compatibilidad.
function getInvoke() {
  if (typeof window === "undefined") return null;
  const t = window.__TAURI__;
  if (!t) return null;
  if (t.core && typeof t.core.invoke === "function") return t.core.invoke;
  if (typeof t.invoke === "function") return t.invoke;
  return null;
}
function getListen() {
  const t = window.__TAURI__;
  if (!t) return null;
  // Tauri 2 expone los plugins en `__TAURI__.event` y `__TAURI__.core`.
  if (t.event && typeof t.event.listen === "function") return t.event.listen;
  if (t.core && typeof t.core.listen === "function") return t.core.listen;
  return null;
}
const invoke = getInvoke();
const listenEv = getListen();
if (!invoke) {
  console.error(
    "[SynapseCortana] window.__TAURI__ no disponible. ¿Olvidaste poner " +
      "app.withGlobalTauri=true en tauri.conf.json?",
  );
}

const state = {
  connected: false,
  gatewayUrl: "http://localhost:18789",
  gatewayToken: "",
  voices: [],
  selectedVoice: null,
  ttsLoaded: false,
  ttsSampleRate: 0,
  autoSpeak: true,
  // FASE 2.4.B: lista de sesiones del gateway (cache en memoria).
  sessions: [],
  sessionKey: "agent:main:main",
};

const elements = {
  // Pestañas
  tabs: document.querySelectorAll(".tab"),
  tabPanels: document.querySelectorAll(".tab-panel"),
  // Config
  gatewayUrl: document.getElementById("gateway-url"),
  gatewayToken: document.getElementById("gateway-token"),
  btnTest: document.getElementById("btn-test"),
  btnConnect: document.getElementById("btn-connect"),
  sessionKey: document.getElementById("session-key"),
  sessionKeyCustom: document.getElementById("session-key-custom"),
  btnUseCustomSession: document.getElementById("btn-use-custom-session"),
  btnRefreshSessions: document.getElementById("btn-refresh-sessions"),
  ttsVoice: document.getElementById("tts-voice"),
  btnTtsTest: document.getElementById("btn-tts-test"),
  chkAutoSpeak: document.getElementById("chk-auto-speak"),
  sttModel: document.getElementById("stt-model"),
  sttMic: document.getElementById("stt-mic"),
  chkAutoSendAfterDictation: document.getElementById(
    "chk-auto-send-after-dictation",
  ),
  btnResetSettings: document.getElementById("btn-reset-settings"),
  // Status bar
  statusDot: document.getElementById("status-dot"),
  statusText: document.getElementById("status-text"),
  ttsStatusText: document.getElementById("tts-status-text"),
  // Chat
  chat: document.getElementById("chat"),
  messageInput: document.getElementById("message-input"),
  btnSend: document.getElementById("btn-send"),
  btnMic: document.getElementById("btn-mic"),
};

// ============================================
// AUDIO (reproducir WAV base64 con <audio>)
// ============================================

/**
 * Reproduce un audio WAV recibido como base64 desde el backend.
 */
function playBase64Wav(b64) {
  return new Promise((resolve, reject) => {
    try {
      const bin = atob(b64);
      const len = bin.length;
      const bytes = new Uint8Array(len);
      for (let i = 0; i < len; i++) bytes[i] = bin.charCodeAt(i);
      const blob = new Blob([bytes], { type: "audio/wav" });
      const url = URL.createObjectURL(blob);
      const audio = new Audio(url);
      audio.onended = () => {
        URL.revokeObjectURL(url);
        resolve();
      };
      audio.onerror = (e) => {
        URL.revokeObjectURL(url);
        reject(new Error("error reproduciendo audio: " + e));
      };
      audio.play().catch((e) => {
        URL.revokeObjectURL(url);
        reject(e);
      });
    } catch (e) {
      reject(e);
    }
  });
}

// ============================================
// PESTAÑAS (FASE 2.4.A)
// ============================================

function switchTab(tabName) {
  elements.tabs.forEach((tab) => {
    const isActive = tab.dataset.tab === tabName;
    tab.classList.toggle("active", isActive);
  });
  elements.tabPanels.forEach((panel) => {
    const isActive = panel.dataset.panel === tabName;
    panel.classList.toggle("active", isActive);
  });
  // Persistir la pestaña activa.
  scheduleSaveSettings();
}

// ============================================
// STATUS BAR
// ============================================

function updateConnectionStatus(connected, error = false) {
  state.connected = connected;
  elements.statusDot.classList.remove("connected", "disconnected", "error");
  if (connected) {
    elements.statusDot.classList.add("connected");
    elements.statusText.textContent = "Conectado a OpenClaw";
  } else if (error) {
    elements.statusDot.classList.add("error");
    elements.statusText.textContent = "Error de conexión";
  } else {
    elements.statusDot.classList.add("disconnected");
    elements.statusText.textContent = "Desconectado";
  }
}

function updateTtsStatusText() {
  elements.ttsStatusText.classList.remove("ready", "loading");
  if (state.ttsLoaded && state.selectedVoice) {
    elements.ttsStatusText.textContent = `🔊 TTS listo (${state.ttsSampleRate} Hz)`;
    elements.ttsStatusText.classList.add("ready");
  } else if (state.selectedVoice) {
    elements.ttsStatusText.textContent = "⏳ Descargando voz…";
    elements.ttsStatusText.classList.add("loading");
  } else {
    elements.ttsStatusText.textContent = "🔇 TTS sin inicializar";
  }
}

// ============================================
// CHAT UI
// ============================================

// Buffer de los últimos 10 mensajes de Cortana para poder
// reproducirlos de nuevo con el botón 🔊.
const cortanaHistory = [];
const MAX_HISTORY = 10;

function addMessage(content, type = "user") {
  const messageDiv = document.createElement("div");
  messageDiv.className = `message ${type}`;
  messageDiv.textContent = content;
  // Si es un mensaje de Cortana, añadir un botón 🔊 para re-reproducir.
  if (type === "cortana" && content && content.trim().length > 0) {
    const replayBtn = document.createElement("button");
    replayBtn.className = "btn-replay";
    replayBtn.title = "Reproducir de nuevo";
    replayBtn.textContent = "🔊";
    replayBtn.addEventListener("click", (e) => {
      e.stopPropagation();
      speakText(content);
    });
    messageDiv.appendChild(replayBtn);
    // Guardar en historial (solo si no es muy corto).
    if (content.trim().length > 5) {
      cortanaHistory.push(content);
      if (cortanaHistory.length > MAX_HISTORY) {
        cortanaHistory.shift();
      }
    }
  }
  elements.chat.appendChild(messageDiv);
  elements.chat.scrollTop = elements.chat.scrollHeight;
  return messageDiv;
}

// Limpia el texto para ser enviado a TTS: quita markdown y arregla
// espacios dobles alrededor de puntuación (artefacto de sherpa-onnx
// que rompe palabras como "recorr erme" en "recorrerme").
function cleanForTTS(text) {
  if (!text) return "";
  let t = text;
  // Quitar énfasis markdown: **bold**, *italic*, __bold__, _italic_.
  t = t.replace(/\*\*([^*]+)\*\*/g, "$1");
  t = t.replace(/\*([^*]+)\*/g, "$1");
  t = t.replace(/__([^_]+)__/g, "$1");
  t = t.replace(/_([^_]+)_/g, "$1");
  // Quitar headers markdown.
  t = t.replace(/^#{1,6}\s+/gm, "");
  // Quitar blockquotes.
  t = t.replace(/^>\s*/gm, "");
  // Quitar links [texto](url) -> texto.
  t = t.replace(/\[([^\]]+)\]\(([^)]+)\)/g, "$1");
  // Quitar code spans.
  t = t.replace(/`([^`]+)`/g, "$1");
  // Colapsar espacios dobles/triples que sherpa-onnx inserta entre sílabas.
  t = t.replace(/\s{2,}/g, " ");
  // Quitar espacio antes de signos de puntuación.
  t = t.replace(/\s+([,.!?;:)])/g, "$1");
  // Añadir espacio después de signo de puntuación si no hay.
  t = t.replace(/([,.!?;:])([A-Za-záéíóúñ])/g, "$1 $2");
  // Trim.
  t = t.trim();
  return t;
}

function addSystemMessage(content) {
  return addMessage(content, "system");
}

function showTypingIndicator() {
  if (document.getElementById("typing-indicator")) return;
  const indicator = document.createElement("div");
  indicator.className = "typing";
  indicator.id = "typing-indicator";
  indicator.innerHTML = "<span></span><span></span><span></span>";
  elements.chat.appendChild(indicator);
  elements.chat.scrollTop = elements.chat.scrollHeight;
}

function hideTypingIndicator() {
  const indicator = document.getElementById("typing-indicator");
  if (indicator) indicator.remove();
}

// ============================================
// TTS
// ============================================

async function loadVoiceCatalog() {
  try {
    const voices = await invoke("tts_list_voices");
    state.voices = voices;
    elements.ttsVoice.innerHTML = "";
    for (const v of voices) {
      const opt = document.createElement("option");
      opt.value = v.id;
      opt.textContent = `${v.label} (~${v.size_mb_approx} MB)`;
      elements.ttsVoice.appendChild(opt);
    }
    const defaultVoice = voices.find((v) => v.id === "es_AR-daniela-high");
    if (defaultVoice) {
      elements.ttsVoice.value = defaultVoice.id;
      state.selectedVoice = defaultVoice.id;
    }
    elements.btnTtsTest.disabled = voices.length === 0;
    updateTtsStatusText();
    console.log("[SynapseCortana] catálogo de voces:", voices);
  } catch (e) {
    console.error("[SynapseCortana] tts_list_voices:", e);
    elements.ttsVoice.innerHTML = `<option value="">⚠️ ${e}</option>`;
  }
}

async function selectVoice(voiceId) {
  if (!voiceId) return;
  state.selectedVoice = voiceId;
  state.ttsLoaded = false;
  updateTtsStatusText();
  try {
    const status = await invoke("tts_set_voice", { voiceId });
    state.ttsLoaded = status.loaded;
    state.ttsSampleRate = status.sample_rate;
    updateTtsStatusText();
    console.log("[SynapseCortana] voz cargada:", status);
    scheduleSaveSettings();
  } catch (e) {
    console.error("[SynapseCortana] tts_set_voice:", e);
    addSystemMessage(`❌ Error cargando voz: ${e}`);
  }
}

async function testTts() {
  if (!state.selectedVoice) {
    addSystemMessage("⚠️ Selecciona una voz primero");
    return;
  }
  const phrase =
    "Hola, soy Cortana. Esta es una prueba de la voz " +
    (state.voices.find((v) => v.id === state.selectedVoice)?.label || "");
  elements.btnTtsTest.disabled = true;
  elements.btnTtsTest.textContent = "⏳ Sintetizando...";
  try {
    const res = await invoke("tts_synthesize", {
      text: phrase,
      voiceId: state.selectedVoice,
    });
    console.log("[SynapseCortana] TTS OK:", {
      duration_ms: res.duration_ms,
      sample_rate: res.sample_rate,
      num_samples: res.num_samples,
    });
    addSystemMessage(`🔊 Reproduciendo TTS (${res.duration_ms} ms)`);
    await playBase64Wav(res.audio_base64);
  } catch (e) {
    console.error("[SynapseCortana] tts_synthesize:", e);
    addSystemMessage(`❌ Error TTS: ${e}`);
  } finally {
    elements.btnTtsTest.disabled = false;
    elements.btnTtsTest.textContent = "🔊 Probar voz";
  }
}

// ============================================
// COLA TTS CON CACHÉ
// ============================================
//
// Cuando llega un nuevo mensaje, el audio del mensaje anterior se
// interrumpe y el nuevo se reproduce a continuación (sin esperar a
// que termine el anterior). Además, mantenemos un caché: si el
// mismo texto se vuelve a solicitar (p. ej. al pulsar "re-reproducir"
// 🔊), no volvemos a llamar al backend TTS: reutilizamos el audio
// en base64 que ya generamos.

const ttsCache = new Map(); // textHash -> audioBase64
const TTS_CACHE_MAX = 32;

function hashText(text) {
  // Hash simple (djb2).
  let h = 5381;
  for (let i = 0; i < text.length; i++) {
    h = ((h << 5) + h + text.charCodeAt(i)) | 0;
  }
  return `tts_${h}`;
}

let currentTtsAudio = null; // Referencia al <audio> actual, para poder interrumpirlo.

function playBase64WavImmediate(b64) {
  return new Promise((resolve) => {
    try {
      const bin = atob(b64);
      const bytes = new Uint8Array(bin.length);
      for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
      const blob = new Blob([bytes], { type: "audio/wav" });
      const url = URL.createObjectURL(blob);
      const audio = new Audio(url);
      currentTtsAudio = audio;
      audio.onended = () => {
        URL.revokeObjectURL(url);
        if (currentTtsAudio === audio) currentTtsAudio = null;
        resolve();
      };
      audio.onerror = () => {
        URL.revokeObjectURL(url);
        if (currentTtsAudio === audio) currentTtsAudio = null;
        resolve();
      };
      audio.play().catch((e) => {
        console.error("[SynapseCortana] audio.play:", e);
        if (currentTtsAudio === audio) currentTtsAudio = null;
        resolve();
      });
    } catch (e) {
      console.error("[SynapseCortana] playBase64WavImmediate:", e);
      resolve();
    }
  });
}

async function enqueueTts(audioBase64, text) {
  // Si hay un audio sonando, lo interrumpimos.
  if (currentTtsAudio) {
    try {
      currentTtsAudio.pause();
      currentTtsAudio.currentTime = 0;
    } catch (e) {
      console.warn("[SynapseCortana] no se pudo interrumpir audio:", e);
    }
  }
  await playBase64WavImmediate(audioBase64);
}

async function speakText(text) {
  if (!state.selectedVoice) {
    console.warn("[SynapseCortana] TTS no inicializado, no se reproduce");
    return;
  }
  if (!state.autoSpeak) return;
  // Limpiar markdown y arreglar espacios antes de enviar a TTS.
  const cleaned = cleanForTTS(text);
  if (!cleaned) return;

  const cacheKey = hashText(cleaned);

  // 1) Caché en memoria (LRU, sobrevive solo a esta sesión).
  if (ttsCache.has(cacheKey)) {
    addSystemMessage("🔊 Reproduciendo TTS (caché memoria)");
    await enqueueTts(ttsCache.get(cacheKey), cleaned);
    return;
  }

  // 2) Caché persistente en disco (sobrevive a reinicios). Es la
  // mejora principal de FASE 2.5: si la misma frase se vuelve a
  // pedir (ej. al pulsar "🔊 Reproducir de nuevo" o por re-síntesis
  // de un saludo frecuente), NO se vuelve a llamar al motor TTS.
  try {
    const cached = await invoke("tts_cache_lookup", {
      text: cleaned,
      voiceId: state.selectedVoice,
    });
    if (cached) {
      // Guardar también en memoria para el próximo uso.
      if (ttsCache.size >= TTS_CACHE_MAX) {
        const firstKey = ttsCache.keys().next().value;
        ttsCache.delete(firstKey);
      }
      ttsCache.set(cacheKey, cached);
      addSystemMessage("🔊 Reproduciendo TTS (caché disco)");
      await enqueueTts(cached, cleaned);
      return;
    }
  } catch (e) {
    console.warn("[SynapseCortana] tts_cache_lookup:", e);
  }

  // 3) No hay caché: sintetizar con el motor TTS.
  try {
    const res = await invoke("tts_synthesize", {
      text: cleaned,
      voiceId: state.selectedVoice,
    });
    // Guardar en caché de memoria.
    if (ttsCache.size >= TTS_CACHE_MAX) {
      const firstKey = ttsCache.keys().next().value;
      ttsCache.delete(firstKey);
    }
    ttsCache.set(cacheKey, res.audio_base64);
    // Guardar también en disco (fire-and-forget para no bloquear).
    invoke("tts_cache_store", {
      text: cleaned,
      voiceId: state.selectedVoice,
      audioBase64: res.audio_base64,
    }).catch((e) => console.warn("[SynapseCortana] tts_cache_store:", e));
    await enqueueTts(res.audio_base64, cleaned);
  } catch (e) {
    console.error("[SynapseCortana] speakText:", e);
  }
}

// ============================================
// GATEWAY: CONEXIÓN
// ============================================

async function testConnection() {
  elements.btnTest.disabled = true;
  elements.btnTest.textContent = "Probando...";
  try {
    const url = elements.gatewayUrl.value.trim();
    state.gatewayUrl = url;
    await invoke("set_gateway_url", { url });
    const connected = await invoke("check_gateway_connection");
    updateConnectionStatus(connected, !connected);
    if (connected) {
      addSystemMessage("✅ Gateway de OpenClaw accesible");
    } else {
      addSystemMessage("❌ Gateway no responde");
    }
  } catch (e) {
    console.error("[SynapseCortana] Error:", e);
    addSystemMessage("❌ Error: " + e);
    updateConnectionStatus(false, true);
  } finally {
    elements.btnTest.disabled = false;
    elements.btnTest.textContent = "Test";
  }
}

async function connectToGateway() {
  elements.btnConnect.disabled = true;
  elements.btnConnect.textContent = "Conectando...";
  try {
    const url = elements.gatewayUrl.value.trim();
    const token = elements.gatewayToken.value.trim();
    state.gatewayUrl = url;
    state.gatewayToken = token;
    await invoke("set_gateway_url", { url });
    await invoke("set_gateway_token", { token });
    console.log("[SynapseCortana] Conectando a WebSocket...");
    const info = await invoke("connect_to_gateway");
    const connected = info !== null && info !== undefined;
    updateConnectionStatus(connected);
    if (connected) {
      const connId = info && info.conn_id ? ` (${info.conn_id})` : "";
      const protocol =
        info && info.protocol ? ` protocolo v${info.protocol}` : "";
      addSystemMessage(
        `✅ Conectado al Gateway via WebSocket${protocol}${connId}`,
      );
      startEventPolling();
      // FASE 2.4.B: cargar lista de sesiones tras conectar.
      await refreshSessions();
    } else {
      addSystemMessage("❌ No se pudo conectar");
    }
  } catch (e) {
    console.error("[SynapseCortana] Error:", e);
    addSystemMessage("❌ Error de conexión: " + e);
    updateConnectionStatus(false, true);
  } finally {
    elements.btnConnect.disabled = false;
    elements.btnConnect.textContent = "Conectar";
  }
}

let eventPollingHandle = null;

function startEventPolling() {
  if (eventPollingHandle !== null) return;
  eventPollingHandle = setInterval(async () => {
    try {
      const events = await invoke("poll_gateway_events");
      if (Array.isArray(events) && events.length > 0) {
        for (const ev of events) {
          handleGatewayEvent(ev);
        }
      }
    } catch (e) {
      console.error("[SynapseCortana] poll_gateway_events:", e);
    }
  }, 1000);
  if (streamTimeoutHandle === null) {
    streamTimeoutHandle = setInterval(flushIdleSessions, 500);
  }
}

let streamTimeoutHandle = null;
const STREAM_IDLE_MS = 2500;

function flushIdleSessions() {
  const now = Date.now();
  for (const [key, s] of streamState.sessions.entries()) {
    if (now - s.lastChunkAt > STREAM_IDLE_MS) {
      flushSession(key);
    }
  }
  for (const key of Array.from(streamState.ignoreKeys)) {
    if (!streamState.sessions.has(key)) {
      streamState.ignoreKeys.delete(key);
    }
  }
}

// ============================================
// ACUMULADOR DE STREAMING (FASE 2.3)
// ============================================

const streamState = {
  sessions: new Map(),
  ignoreKeys: new Set(),
  // sessionKey's cuyo texto+audio ya fueron entregados a la UI vía
  // `chat_and_speak`. Cuando `flushSession` los procesa, NO vuelve a
  // reproducir audio ni a mostrar el texto (evita la duplicación que
  // vimos en producción: el texto aparecía dos veces y el audio se
  // reproducía periódicamente sin acción del usuario).
  consumedKeys: new Set(),
  audioBusy: false,
};

function getSession(key) {
  let s = streamState.sessions.get(key);
  if (!s) {
    s = { accumulated: "", lastChunkAt: 0 };
    streamState.sessions.set(key, s);
  }
  return s;
}

function resetSession(key) {
  streamState.sessions.delete(key);
}

async function flushSession(key) {
  const s = streamState.sessions.get(key);
  if (!s) return;
  const text = s.accumulated.trim();
  resetSession(key);
  if (streamState.ignoreKeys.has(key)) {
    streamState.ignoreKeys.delete(key);
    return;
  }
  // FASE 2.4 fix duplicación (v2): si este sessionKey ya pasó por
  // `chat_and_speak`, el texto y audio ya fueron entregados. NO
  // volvemos a mostrarlos ni a sintetizar audio otra vez. NO borramos
  // `consumedKeys` aquí — el caller (`sendMessage`) lo gestiona para
  // evitar una race donde se borra antes de tiempo y los chunks
  // tardíos re-entran.
  if (streamState.consumedKeys.has(key)) {
    return;
  }
  if (!text) return;
  // Deduplicación adicional: si este texto ya está en `cortanaHistory`
  // (los últimos N mensajes de Cortana), NO lo mostramos otra vez.
  if (cortanaHistory.includes(text)) {
    return;
  }
  hideTypingIndicator();
  addMessage(text, "cortana");
  streamState.audioBusy = true;
  try {
    await speakText(text);
  } finally {
    streamState.audioBusy = false;
  }
}

function handleStreamingChunk(eventName, payload) {
  const sessionKey = payload.sessionKey || payload.session_key || "default";
  if (streamState.ignoreKeys.has(sessionKey)) {
    return true;
  }
  const delta =
    (payload.data && payload.data.delta) ||
    payload.delta ||
    payload.message ||
    payload.content ||
    null;
  // Texto acumulado que envía el gateway (contiene TODO lo recibido
  // hasta el momento, incluyendo el `delta` actual). Si está presente,
  // SIEMPRE sobrescribe el acumulado local para evitar duplicación.
  const accumulated =
    (payload.data && payload.data.text) || payload.text || null;
  const terminalEvents = new Set([
    "chat.done",
    "agent.done",
    "chat.abort",
    "agent.abort",
    "session.done",
  ]);
  if (terminalEvents.has(eventName)) {
    flushSession(sessionKey);
    return true;
  }
  const isChunkEvent =
    eventName === "chat" ||
    eventName === "agent" ||
    eventName === "chat.message" ||
    eventName === "agent.message" ||
    eventName === "chat.delta" ||
    eventName === "session.message" ||
    (typeof accumulated === "string" && accumulated.length > 0) ||
    (typeof delta === "string" && delta.length > 0);
  if (!isChunkEvent) return false;
  const s = getSession(sessionKey);
  if (typeof accumulated === "string" && accumulated.length > 0) {
    // El gateway envía el texto completo acumulado; sobrescribimos.
    // Pero solo si es al menos tan largo como el actual (monotonía).
    // Si es más corto, es probablemente un chunk parcial viejo: lo
    // ignoramos para no cortar párrafos a la mitad.
    if (accumulated.length >= s.accumulated.length) {
      s.accumulated = accumulated;
    }
  } else if (typeof delta === "string" && delta.length > 0) {
    // Solo llega `delta` (sin `text` acumulado); concatenamos.
    s.accumulated += delta;
  }
  s.lastChunkAt = Date.now();
  return true;
}

async function handleGatewayEvent(ev) {
  if (!ev || !ev.event) return;
  const eventName = ev.event;
  const payload = ev.payload || {};
  // FASE 2.4.B: las respuestas a RPCs del gateway llegan como event="res".
  // El backend las inyecta al inbox, pero el frontend NO debe procesarlas
  // como chunks de streaming. Las dejamos pasar silenciosamente (el backend
  // las lee directamente desde el inbox vía `send_request_and_wait`).
  if (eventName === "res") return;
  if (eventName === "gateway:disconnected") {
    updateConnectionStatus(false, true);
    stopEventPolling();
    addSystemMessage("⚠️ Conexión con el gateway perdida");
    for (const key of Array.from(streamState.sessions.keys())) {
      flushSession(key);
    }
    return;
  }
  if (handleStreamingChunk(eventName, payload)) return;
  console.log("[SynapseCortana] event ignorado:", eventName, payload);
}

function stopEventPolling() {
  if (eventPollingHandle !== null) {
    clearInterval(eventPollingHandle);
    eventPollingHandle = null;
  }
  if (streamTimeoutHandle !== null) {
    clearInterval(streamTimeoutHandle);
    streamTimeoutHandle = null;
  }
}

// ============================================
// SESIONES (FASE 2.4.B)
// ============================================

async function refreshSessions() {
  if (!state.connected) {
    addSystemMessage("⚠️ Conéctate al gateway primero");
    return;
  }
  elements.btnRefreshSessions.disabled = true;
  try {
    const sessions = await invoke("gateway_list_sessions");
    state.sessions = sessions || [];
    populateSessionSelect();
  } catch (e) {
    console.error("[SynapseCortana] gateway_list_sessions:", e);
    addSystemMessage("❌ Error listando sesiones: " + e);
  } finally {
    elements.btnRefreshSessions.disabled = false;
  }
}

function populateSessionSelect() {
  const sel = elements.sessionKey;
  sel.innerHTML = "";
  // Siempre ofrecemos un fallback al sessionKey por defecto, incluso si
  // el gateway no devolvió nada.
  const defaults = [
    { key: "agent:main:main", label: "agent:main:main (default)" },
  ];
  const seen = new Set();
  // Combinar defaults + sesiones del gateway, deduplicando por key.
  // ANTES había un bug: el bucle llenaba `seen` pero `opts` después
  // concatenaba `defaults + allRows` sin filtrar, así que `agent:main:main`
  // aparecía dos veces si también venía en `state.sessions`.
  const merged = [...defaults];
  for (const s of defaults) seen.add(s.key);
  for (const s of state.sessions || []) {
    if (!s || !s.key) continue;
    if (seen.has(s.key)) continue;
    seen.add(s.key);
    merged.push(s);
  }
  for (const s of merged) {
    const opt = document.createElement("option");
    opt.value = s.key;
    const label =
      s.label && s.label !== s.key ? `${s.label} — ${s.key}` : s.key;
    opt.textContent = label;
    sel.appendChild(opt);
  }
  sel.disabled = false;
  // Restaurar selección si está en la lista; si no, dejar el default.
  if (![...sel.options].some((o) => o.value === state.sessionKey)) {
    state.sessionKey = "agent:main:main";
  }
  sel.value = state.sessionKey;
  elements.sessionKeyCustom.value = "";
}

async function useCustomSession() {
  const raw = elements.sessionKeyCustom.value.trim();
  if (!raw) {
    addSystemMessage("⚠️ Pega un sessionKey en el campo");
    return;
  }
  elements.btnUseCustomSession.disabled = true;
  try {
    const resolved = await invoke("gateway_resolve_session", { input: raw });
    state.sessionKey = resolved;
    if (![...elements.sessionKey.options].some((o) => o.value === resolved)) {
      const opt = document.createElement("option");
      opt.value = resolved;
      opt.textContent = `${resolved} (custom)`;
      elements.sessionKey.appendChild(opt);
    }
    elements.sessionKey.value = resolved;
    scheduleSaveSettings();
    addSystemMessage(`✅ Sesión activa: ${resolved}`);
  } catch (e) {
    addSystemMessage(`❌ No se pudo resolver "${raw}": ${e}`);
  } finally {
    elements.btnUseCustomSession.disabled = false;
  }
}

// ============================================
// SETTINGS PERSISTENTES (FASE 2.4.A)
// ============================================

let saveSettingsTimer = null;

function scheduleSaveSettings() {
  if (saveSettingsTimer) clearTimeout(saveSettingsTimer);
  saveSettingsTimer = setTimeout(saveSettingsNow, 300);
}

async function saveSettingsNow() {
  if (!invoke) return;
  try {
    const settings = {
      gateway_url: elements.gatewayUrl.value.trim() || "http://localhost:18789",
      gateway_token: elements.gatewayToken.value,
      voice_id: state.selectedVoice || "es_AR-daniela-high",
      auto_speak: elements.chkAutoSpeak.checked,
      session_key: state.sessionKey,
      last_tab: getActiveTab(),
      stt_model_id: elements.sttModel ? elements.sttModel.value : "",
      auto_send_after_dictation: elements.chkAutoSendAfterDictation.checked,
      // FASE 2.5: timeouts configurables. Si el input está vacío o
      // inválido, usamos los defaults (1500 / 30000 ms).
      silence_timeout_ms: parseIntOr(
        elements.silenceTimeoutMs ? elements.silenceTimeoutMs.value : "",
        1500,
        500,
        10000,
      ),
      overall_timeout_ms: parseIntOr(
        elements.overallTimeoutMs ? elements.overallTimeoutMs.value : "",
        30000,
        5000,
        300000,
      ),
    };
    await invoke("save_settings_cmd", { settings });
  } catch (e) {
    console.error("[SynapseCortana] save_settings_cmd:", e);
  }
}

/// Parsea un entero con valor por defecto y límites [min, max].
function parseIntOr(raw, defaultVal, min, max) {
  if (!raw) return defaultVal;
  const n = parseInt(raw, 10);
  if (Number.isNaN(n)) return defaultVal;
  if (n < min) return min;
  if (n > max) return max;
  return n;
}

function getActiveTab() {
  const active = document.querySelector(".tab.active");
  return active ? active.dataset.tab : "config";
}

async function loadAndApplySettings() {
  if (!invoke) return;
  try {
    const s = await invoke("get_settings");
    state.gatewayUrl = s.gateway_url || "http://localhost:18789";
    state.gatewayToken = s.gateway_token || "";
    state.sessionKey = s.session_key || "agent:main:main";
    state.autoSpeak = !!s.auto_speak;
    elements.gatewayUrl.value = s.gateway_url || "";
    elements.gatewayToken.value = s.gateway_token || "";
    elements.chkAutoSpeak.checked = !!s.auto_speak;
    elements.chkAutoSendAfterDictation.checked = !!s.auto_send_after_dictation;
    if (s.voice_id) state.selectedVoice = s.voice_id;
    if (s.stt_model_id) elements.sttModel.value = s.stt_model_id;
    // FASE 2.5: timeouts configurables.
    if (elements.silenceTimeoutMs)
      elements.silenceTimeoutMs.value = s.silence_timeout_ms ?? 1500;
    if (elements.overallTimeoutMs)
      elements.overallTimeoutMs.value = s.overall_timeout_ms ?? 30000;
    if (s.last_tab) switchTab(s.last_tab);
  } catch (e) {
    console.error("[SynapseCortana] get_settings:", e);
  }
}

async function resetSettings() {
  if (
    !confirm(
      "¿Restablecer toda la configuración? Se perderán URL, token, voz y sesión seleccionadas.",
    )
  )
    return;
  try {
    const defaults = await invoke("reset_settings_cmd");
    state.gatewayUrl = defaults.gateway_url;
    state.gatewayToken = defaults.gateway_token;
    state.sessionKey = defaults.session_key;
    state.autoSpeak = defaults.auto_speak;
    state.selectedVoice = defaults.voice_id;
    elements.gatewayUrl.value = defaults.gateway_url;
    elements.gatewayToken.value = "";
    elements.chkAutoSpeak.checked = defaults.auto_speak;
    elements.chkAutoSendAfterDictation.checked =
      defaults.auto_send_after_dictation;
    elements.sessionKeyCustom.value = "";
    addSystemMessage("🗑️ Configuración restablecida");
  } catch (e) {
    addSystemMessage("❌ Error al restablecer: " + e);
  }
}

// ============================================
// CHAT: ENVIAR MENSAJES
// ============================================

async function sendMessage() {
  const message = elements.messageInput.value.trim();
  if (!message) return;

  addMessage(message, "user");
  elements.messageInput.value = "";
  showTypingIndicator();
  elements.btnSend.disabled = true;

  try {
    if (state.autoSpeak && state.selectedVoice) {
      const sessionKeyPlaceholder = state.sessionKey;
      // FASE 2.4 fix duplicación (v2): marcar consumedKeys ANTES del
      // invoke para que cualquier `flushSession` que se dispare mientras
      // `chat_and_speak` corre (o inmediatamente después) descarte los
      // chunks en lugar de volver a mostrar el texto. Antes se marcaba
      // DESPUÉS del invoke, lo que producía una segunda renderización
      // cuando llegaba `chat.done` entre el finally y el addMessage.
      streamState.ignoreKeys.add(sessionKeyPlaceholder);
      streamState.consumedKeys.add(sessionKeyPlaceholder);
      // Limpiar cualquier acumulación previa de esta sesión para que el
      // `accumulated` local no compita con el `result.agent_text`.
      streamState.sessions.delete(sessionKeyPlaceholder);
      // FASE 3: avisar al avatar que estamos pensando.
      invoke("set_avatar_state", { state: "thinking" }).catch(() => {});
      let result;
      try {
        result = await invoke("chat_and_speak", {
          message,
          voiceId: state.selectedVoice,
          silenceTimeoutMs: 2000,
          overallTimeoutMs: 30000,
        });
      } finally {
        streamState.ignoreKeys.delete(sessionKeyPlaceholder);
      }
      hideTypingIndicator();
      // FASE 3 fix: mostrar el texto ORIGINAL del LLM en el chat (con
      // markdown, poemas, formato, etc.). cleanForTTS se aplica SOLO
      // cuando se envía al motor TTS, no para mostrar en pantalla.
      if (result.agent_text) {
        const originalText = result.agent_text;
        const cleanedForTTS = cleanForTTS(originalText);
        // Deduplicar: si ya mostramos este texto, NO lo añadimos otra vez.
        if (!cortanaHistory.includes(originalText)) {
          addMessage(originalText, "cortana");
        }
        // Guardar el audio en caché usando el hash del texto LIMPIO (el
        // mismo que usa speakText cuando el usuario pulsa 🔊).
        if (result.audio_base64 && cleanedForTTS) {
          const cacheKey = hashText(cleanedForTTS);
          if (ttsCache.size >= TTS_CACHE_MAX) {
            const firstKey = ttsCache.keys().next().value;
            ttsCache.delete(firstKey);
          }
          ttsCache.set(cacheKey, result.audio_base64);
          invoke("tts_cache_store", {
            text: cleanedForTTS,
            voiceId: state.selectedVoice,
            audioBase64: result.audio_base64,
          }).catch((e) => console.warn("[SynapseCortana] tts_cache_store:", e));
        }
      } else {
        addSystemMessage("⚠️ El agente no devolvió texto");
      }
      if (result.audio_base64) {
        invoke("set_avatar_state", { state: "speaking" }).catch(() => {});
        await enqueueTts(result.audio_base64);
        invoke("set_avatar_state", { state: "idle" }).catch(() => {});
      }
      // NO borrar consumedKeys aquí. Si lo borramos, los chunks
      // tardíos del gateway (que pueden llegar hasta 30s después)
      // disparan flushSession y muestran el texto duplicado/intercalado.
      // El consumedKeys se limpia naturalmente cuando el usuario envía
      // un nuevo mensaje (se hace add + delete al inicio de sendMessage).
      console.log(
        "[SynapseCortana] chat_and_speak OK en",
        result.elapsed_ms,
        "ms",
      );
    } else {
      const reqId = await invoke("send_message_to_gateway", { message });
      console.log("[SynapseCortana] reqId:", reqId);
    }
  } catch (e) {
    hideTypingIndicator();
    console.error("[SynapseCortana] Error:", e);
    addSystemMessage("❌ Error: " + e);
  } finally {
    elements.btnSend.disabled = false;
  }
}

// ============================================
// STT (FASE 2.4.C) — funcional end-to-end
// ============================================
//
// Captura audio del micrófono vía cpal + reconocimiento streaming
// sherpa-onnx Zipformer. Los eventos `stt:partial` y `stt:final`
// llegan del backend y actualizan el input en vivo.

async function loadSttCatalog() {
  if (!invoke) return;
  try {
    const models = await invoke("stt_list_models");
    elements.sttModel.innerHTML = "";
    for (const m of models) {
      const opt = document.createElement("option");
      opt.value = m.id;
      opt.textContent = `${m.label} (~${m.size_mb_approx} MB)`;
      elements.sttModel.appendChild(opt);
    }
    // Si hay un stt_model_id en settings, restaurarlo; si no,
    // usar el primer modelo del catálogo.
    const settings = await invoke("get_settings");
    if (
      settings.stt_model_id &&
      models.some((m) => m.id === settings.stt_model_id)
    ) {
      elements.sttModel.value = settings.stt_model_id;
    } else if (models.length > 0) {
      elements.sttModel.value = models[0].id;
    }
    elements.sttModel.disabled = models.length === 0;
    console.log("[SynapseCortana] catálogo STT:", models);
  } catch (e) {
    console.error("[SynapseCortana] stt_list_models:", e);
    elements.sttModel.innerHTML = `<option value="">⚠️ ${e}</option>`;
  }
}

async function loadSttMicrophones() {
  if (!invoke) return;
  try {
    const mics = await invoke("stt_list_microphones");
    const current = await invoke("stt_get_microphone");
    elements.sttMic.innerHTML = "";
    // Opción "default" (vacía = usar default del sistema).
    const defOpt = document.createElement("option");
    defOpt.value = "";
    defOpt.textContent = "Default del sistema";
    elements.sttMic.appendChild(defOpt);
    for (const m of mics) {
      const opt = document.createElement("option");
      opt.value = m.name;
      const sr = m.sample_rate > 0 ? ` @ ${m.sample_rate} Hz` : "";
      const def = m.is_default ? " (default)" : "";
      opt.textContent = `${m.name}${sr}${def}`;
      elements.sttMic.appendChild(opt);
    }
    // Si el usuario tenía uno seleccionado, restaurarlo.
    if (
      current &&
      [...elements.sttMic.options].some((o) => o.value === current)
    ) {
      elements.sttMic.value = current;
    }
    console.log("[SynapseCortana] micrófonos disponibles:", mics);
  } catch (e) {
    console.error("[SynapseCortana] stt_list_microphones:", e);
  }
}

let isSttRecording = false;

async function toggleStt() {
  if (!invoke) return;
  if (!isSttRecording) {
    // Iniciar: el backend abre el stream de cpal y empieza a emitir
    // `stt:partial` / `stt:final`.
    elements.btnMic.classList.add("recording");
    elements.btnMic.disabled = true;
    try {
      const modelId = elements.sttModel.value || undefined;
      await invoke("stt_start", { modelId });
      isSttRecording = true;
      elements.btnMic.textContent = "⏹️";
      elements.messageInput.placeholder = "🎙️ Escuchando...";
      // Borrar lo que haya en el input para mostrar solo el dictado.
      elements.messageInput.value = "";
      addSystemMessage("🎙️ Dictado activo — habla ahora");
      // FASE 3: avisar al avatar que estamos escuchando.
      invoke("set_avatar_state", { state: "listening" }).catch(() => {});
    } catch (e) {
      addSystemMessage("❌ No se pudo iniciar el dictado: " + e);
      elements.btnMic.classList.remove("recording");
    } finally {
      elements.btnMic.disabled = false;
    }
  } else {
    // Detener: el backend cierra el stream y emite el último `stt:final`.
    try {
      await invoke("stt_stop");
    } catch (e) {
      addSystemMessage("❌ Error al detener: " + e);
    }
    isSttRecording = false;
    elements.btnMic.classList.remove("recording");
    elements.btnMic.textContent = "🎙️";
    elements.messageInput.placeholder = "Escribe o pulsa 🎙️ para dictar...";
    // FASE 3: volver a idle.
    invoke("set_avatar_state", { state: "idle" }).catch(() => {});
    // Si hay texto en el input, enviar o dejarlo para revisión.
    if (elements.messageInput.value.trim().length > 0) {
      if (elements.chkAutoSendAfterDictation.checked) {
        sendMessage();
      } else {
        addSystemMessage(
          "✏️ Texto dictado en el input. Revisa y pulsa Enviar.",
        );
      }
    } else {
      addSystemMessage("⏹️ Dictado detenido (sin transcripción)");
    }
  }
}

// Listeners para los eventos de transcripción del backend.
async function setupSttListeners() {
  if (!listenEv) return;
  // Eventos parciales: actualizan el input en vivo.
  await listenEv("stt:partial", (event) => {
    const text = event.payload && event.payload.text;
    if (text) {
      elements.messageInput.value = text;
    }
  });
  // Evento final: marca el texto, opcionalmente envía.
  await listenEv("stt:final", (event) => {
    const text = event.payload && event.payload.text;
    if (text && text.trim().length > 0) {
      elements.messageInput.value = text;
      // NO llamar sendMessage() desde aquí. El auto-send se maneja
      // desde el backend (stt_stop → chat.eval → sendMessage).
      // Si lo llamamos aquí también, el mensaje se envía dos veces.
    }
  });
  // FASE 3: sincronizar el botón mic cuando el dictado se inicia/detiene
  // desde la ventana del avatar (click en el avatar). Este listener SOLO
  // actualiza la UI (botón, placeholder). NO llama sendMessage() — el
  // envío automático se maneja en toggleStt() (botón mic del chat).
  await listenEv("stt:state", (event) => {
    const recording = event.payload && event.payload.recording;
    if (recording) {
      // Dictado iniciado (posiblemente desde el avatar).
      if (!isSttRecording) {
        isSttRecording = true;
        elements.btnMic.classList.add("recording");
        elements.btnMic.textContent = "⏹️";
        elements.messageInput.placeholder = "🎙️ Escuchando...";
        elements.messageInput.value = "";
      }
    } else {
      // Dictado detenido. Solo actualizar UI, NO enviar automáticamente.
      if (isSttRecording) {
        isSttRecording = false;
        elements.btnMic.classList.remove("recording");
        elements.btnMic.textContent = "🎙️";
        elements.messageInput.placeholder = "Escribe o pulsa 🎙️ para dictar...";
        // El texto dictado queda en el input para que el usuario lo revise
        // y envíe manualmente (Enter o botón Enviar).
      }
    }
  });
}

elements.sttModel.addEventListener("change", async () => {
  scheduleSaveSettings();
  // Pre-cargar el modelo seleccionado para que esté listo cuando el
  // usuario pulse 🎙️. Si falla, mostrar el error en el log.
  const id = elements.sttModel.value;
  if (id) {
    try {
      addSystemMessage(`⏳ Cargando modelo STT "${id}"...`);
      await invoke("stt_set_model", { modelId: id });
      addSystemMessage(`✅ Modelo STT "${id}" listo`);
    } catch (e) {
      addSystemMessage(`❌ Error cargando modelo STT: ${e}`);
    }
  }
});

// ============================================
// EVENT LISTENERS
// ============================================

elements.tabs.forEach((tab) =>
  tab.addEventListener("click", () => switchTab(tab.dataset.tab)),
);

elements.btnTest.addEventListener("click", testConnection);
elements.btnConnect.addEventListener("click", connectToGateway);
elements.btnSend.addEventListener("click", sendMessage);
elements.btnTtsTest.addEventListener("click", testTts);
elements.btnRefreshSessions.addEventListener("click", refreshSessions);
elements.btnUseCustomSession.addEventListener("click", useCustomSession);
elements.btnResetSettings.addEventListener("click", resetSettings);
elements.btnMic.addEventListener("click", toggleStt);

elements.ttsVoice.addEventListener("change", (e) => {
  selectVoice(e.target.value);
});
elements.chkAutoSpeak.addEventListener("change", () => scheduleSaveSettings());
elements.chkAutoSendAfterDictation.addEventListener("change", () =>
  scheduleSaveSettings(),
);
elements.gatewayUrl.addEventListener("change", () => {
  state.gatewayUrl = elements.gatewayUrl.value.trim();
  scheduleSaveSettings();
});
elements.gatewayToken.addEventListener("change", () => {
  state.gatewayToken = elements.gatewayToken.value;
  scheduleSaveSettings();
});
elements.sessionKey.addEventListener("change", (e) => {
  state.sessionKey = e.target.value;
  scheduleSaveSettings();
});

// FASE 2.5: listeners para timeouts configurables.
if (elements.silenceTimeoutMs) {
  elements.silenceTimeoutMs.addEventListener("change", () =>
    scheduleSaveSettings(),
  );
}
if (elements.overallTimeoutMs) {
  elements.overallTimeoutMs.addEventListener("change", () =>
    scheduleSaveSettings(),
  );
}

// FASE 2.5: botón para vaciar el caché TTS.
if (elements.btnClearTtsCache) {
  elements.btnClearTtsCache.addEventListener("click", async () => {
    if (
      !confirm(
        "¿Vaciar el caché TTS? Se borrarán todos los WAVs sintetizados y se re-sintetizarán la próxima vez (más lento la primera vez).",
      )
    )
      return;
    try {
      const removed = await invoke("tts_cache_clear");
      addSystemMessage(`🗑️ Caché TTS vaciado (${removed} archivos)`);
      refreshTtsCacheStats();
    } catch (e) {
      addSystemMessage(`❌ Error vaciando caché: ${e}`);
    }
  });
}

async function refreshTtsCacheStats() {
  if (!elements.ttsCacheStats) return;
  try {
    const stats = await invoke("tts_cache_stats");
    elements.ttsCacheStats.textContent = `${stats.count} entradas · ${stats.total_mb.toFixed(2)} MB`;
  } catch (e) {
    elements.ttsCacheStats.textContent = `⚠️ ${e}`;
  }
}

elements.messageInput.addEventListener("keypress", (e) => {
  if (e.key === "Enter" && !e.shiftKey) {
    e.preventDefault();
    sendMessage();
  }
});

// ============================================
// INIT
// ============================================

(async function init() {
  console.log("[SynapseCortana] FASE 2.5 inicializada");
  // 1) Cargar settings persistentes ANTES de cualquier otra cosa.
  await loadAndApplySettings();
  // FASE 2.5: refrescar estadísticas del caché TTS en background.
  refreshTtsCacheStats();
  // 2) Cargar catálogo de voces TTS (la voz seleccionada en settings
  // se aplica tras cargar el catálogo).
  await loadVoiceCatalog();
  await loadSttCatalog();
  await setupSttListeners();
  if (state.selectedVoice) {
    try {
      const status = await invoke("tts_set_voice", {
        voiceId: state.selectedVoice,
      });
      state.ttsLoaded = status.loaded;
      state.ttsSampleRate = status.sample_rate;
      updateTtsStatusText();
      elements.ttsVoice.value = state.selectedVoice;
      console.log("[SynapseCortana] voz pre-cargada:", status);
    } catch (e) {
      console.warn("[SynapseCortana] pre-carga de voz falló:", e);
    }
  }
  // 3) Mensaje de bienvenida (solo si el chat está vacío).
  if (elements.chat.children.length === 0) {
    addMessage(
      "¡Hola! Soy Synapse Cortana. Configura el gateway en la pestaña " +
        "⚙️ Configuración y pulsa Conectar.",
      "cortana",
    );
  }
})();
