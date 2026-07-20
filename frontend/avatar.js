// SynapseCortana - Avatar 3D (FASE 3.2)
//
// Carga cortana_completa.glb con Three.js usando los materiales originales.
// El avatar se muestra de FRENTE al arrancar, sin rotación automática.
// El usuario controla el giro manualmente.
//
// Interacción:
// - Click izquierdo simple: inicia/detiene dictado.
// - Triple-click (<600ms): cierra el avatar.
// - Click derecho: toggle chat.
// - Rueda del mouse (scroll): gira el modelo manualmente.
// - Drag (arrastrar): mueve la ventana por la pantalla.

import * as THREE from "./vendor/three.module.js";
import { GLTFLoader } from "./vendor/GLTFLoader.js";

console.log("[avatar] Three.js version:", THREE.REVISION);

const { invoke } = window.__TAURI__?.core ?? {};
const { listen } = window.__TAURI__?.event ?? {};

if (!invoke) {
  document.getElementById("loading").innerHTML =
    "<div>⚠️ Tauri IPC no disponible</div>";
}

// ============================================
// CONFIG
// ============================================

const MODEL_PATH = "assets/cortana_completa.glb";

// ============================================
// ESTADO
// ============================================

let avatarState = "idle";
let model = null;
let mixer = null;
let clock = new THREE.Clock();
let keyLight = null;
let rimLight = null;
let fillLight = null;

// Movimiento natural.
let breathPhase = 0;
let swayPhase = 0;
let headTiltPhase = 0;
let baseY = 0;

// Rotación manual del usuario.
let userRotationY = 0; // rotación acumulada por el usuario (radianes).
let autoIdleSway = true; // sway sutil automático en idle.

// ============================================
// THREE.JS
// ============================================

const canvas = document.getElementById("avatar-canvas");
const renderer = new THREE.WebGLRenderer({
  canvas: canvas,
  alpha: true,
  antialias: true,
  powerPreference: "low-power",
});
renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
renderer.setSize(window.innerWidth, window.innerHeight);
renderer.outputColorSpace = THREE.SRGBColorSpace;
renderer.toneMapping = THREE.ACESFilmicToneMapping;
renderer.toneMappingExposure = 1.0;

const scene = new THREE.Scene();

const camera = new THREE.PerspectiveCamera(
  30,
  window.innerWidth / window.innerHeight,
  0.1,
  100,
);

// ============================================
// LUCES
// ============================================

const ambient = new THREE.AmbientLight(0xffffff, 0.5);
scene.add(ambient);

keyLight = new THREE.DirectionalLight(0xffffff, 1.2);
keyLight.position.set(1.5, 2.5, 2.0);
scene.add(keyLight);

fillLight = new THREE.DirectionalLight(0xffeedd, 0.4);
fillLight.position.set(-2, 1, 1.5);
scene.add(fillLight);

rimLight = new THREE.DirectionalLight(0xddeeff, 0.6);
rimLight.position.set(-1, 1.5, -2);
scene.add(rimLight);

// ============================================
// CARGAR MODELO
// ============================================

const loader = new GLTFLoader();
loader.load(
  MODEL_PATH,
  (gltf) => {
    model = gltf.scene;

    // Conservar materiales originales.
    model.traverse((child) => {
      if (child.isMesh) {
        child.castShadow = false;
        child.receiveShadow = false;
        if (child.material && child.material.map) {
          child.material.map.colorSpace = THREE.SRGBColorSpace;
        }
        if (child.material) {
          child.material.needsUpdate = true;
        }
      }
    });

    // Calcular bounding box.
    const box = new THREE.Box3().setFromObject(model);
    const size = box.getSize(new THREE.Vector3());

    // Escalar por altura para ver todo el cuerpo.
    const targetHeight = 2.2;
    const scale = targetHeight / size.y;
    model.scale.setScalar(scale);

    // Recalcular.
    const scaledBox = new THREE.Box3().setFromObject(model);
    const scaledCenter = scaledBox.getCenter(new THREE.Vector3());
    const scaledSize = scaledBox.getSize(new THREE.Vector3());

    // Centrar y poner base en y=0.
    model.position.x -= scaledCenter.x;
    model.position.y -= scaledBox.min.y;
    model.position.z -= scaledCenter.z;

    // El modelo arranca DE FRENTE (rotation.y = 0).
    model.rotation.y = 0;
    baseY = model.position.y;

    // Cámara para ver todo el cuerpo. La distancia se calcula para
    // que el modelo quepa TANTO vertical como horizontalmente.
    const modelHeight = scaledSize.y;
    const modelWidth = scaledSize.x;
    const aspect = window.innerWidth / window.innerHeight;
    const fovRad = (camera.fov * Math.PI) / 180;
    // Distancia para que la altura quepa.
    const distForHeight = modelHeight / (2 * Math.tan(fovRad / 2));
    // Distancia para que el ancho quepa (depende del aspect ratio).
    const fovHorizontal = 2 * Math.atan(Math.tan(fovRad / 2) * aspect);
    const distForWidth = modelWidth / (2 * Math.tan(fovHorizontal / 2));
    // Usar la mayor distancia para que ambos dimensiones quepan.
    const distance = Math.max(distForHeight, distForWidth) * 1.15;
    camera.position.set(0, modelHeight / 2, distance);
    camera.lookAt(0, modelHeight / 2, 0);

    scene.add(model);

    // FASE 3: redimensionar la ventana del avatar al tamaño del modelo
    // para minimizar el área transparente. La altura de la ventana se
    // mantiene, y el ancho se ajusta a la proporción del modelo + margen.
    const modelAspect = modelWidth / modelHeight;
    const windowWidth = Math.max(
      100,
      Math.round(window.innerHeight * modelAspect * 1.25), // 25% margen
    );
    const windowHeight = window.innerHeight;
    invoke("resize_avatar_window", {
      width: windowWidth,
      height: windowHeight,
    }).catch(() => {});

    document.getElementById("loading").style.display = "none";
    console.log(
      "[avatar] Modelo cargado de frente. Altura:",
      modelHeight.toFixed(2),
    );

    if (gltf.animations.length > 0) {
      mixer = new THREE.AnimationMixer(model);
      gltf.animations.forEach((clip) => {
        mixer.clipAction(clip).play();
      });
    }
  },
  (progress) => {
    if (progress.total) {
      console.log(
        `[avatar] Cargando: ${Math.round((progress.loaded / progress.total) * 100)}%`,
      );
    }
  },
  (error) => {
    console.error("[avatar] Error:", error);
    document.getElementById("loading").innerHTML =
      "<div>⚠️ Error cargando avatar</div>";
  },
);

// ============================================
// RENDER LOOP
// ============================================

function animate() {
  requestAnimationFrame(animate);
  const delta = clock.getDelta();
  const t = clock.getElapsedTime();

  if (model) {
    // La rotación Y base es la que el usuario controla con scroll/drag.
    // Añadimos sway sutil encima de esa rotación base.
    let swayY = 0;

    switch (avatarState) {
      case "idle": {
        // Respiración suave.
        breathPhase += delta * 1.5;
        model.position.y = baseY + Math.sin(breathPhase) * 0.015;

        // Sway lateral muy suave.
        swayPhase += delta * 0.5;
        model.rotation.z = Math.sin(swayPhase) * 0.015;

        // Sway Y muy sutil (no rotación completa, solo oscilación).
        if (autoIdleSway) {
          swayY = Math.sin(t * 0.3) * 0.08; // ~5° oscilación
        }

        // Head tilt ocasional.
        headTiltPhase += delta * 0.13;
        model.rotation.x = Math.sin(headTiltPhase) * 0.05;

        keyLight.intensity = 1.2;
        rimLight.intensity = 0.6;
        break;
      }

      case "listening": {
        // Quieta, atenta. Levemente inclinada adelante.
        model.position.y = baseY + Math.sin(t * 2.0) * 0.01;
        model.rotation.z = Math.sin(t * 0.8) * 0.02;
        model.rotation.x = 0.03;

        keyLight.intensity = 1.5;
        rimLight.intensity = 0.9;
        break;
      }

      case "thinking": {
        // Mirando arriba/lados.
        model.position.y = baseY + 0.02;
        model.rotation.z = Math.sin(t * 1.2) * 0.04;
        model.rotation.x = -0.04;

        // Oscilación Y un poco más amplia (procesando).
        swayY = Math.sin(t * 0.8) * 0.15;

        keyLight.intensity = 1.0;
        rimLight.intensity = 0.4;
        break;
      }

      case "speaking": {
        // Movimiento animado al hablar.
        swayPhase += delta * 2.0;
        model.rotation.z = Math.sin(swayPhase) * 0.03;

        breathPhase += delta * 3.0;
        model.position.y = baseY + Math.sin(breathPhase) * 0.025;

        // Gesto lateral ocasional.
        model.rotation.x = Math.sin(t * 1.5) * 0.02;

        // Sway Y suave mientras habla.
        swayY = Math.sin(t * 0.6) * 0.1;

        keyLight.intensity = 1.3;
        rimLight.intensity = 0.7;
        break;
      }
    }

    // Aplicar rotación: base del usuario + sway sutil del estado.
    model.rotation.y = userRotationY + swayY;
  }

  if (mixer) mixer.update(delta);
  renderer.render(scene, camera);
}

animate();

// ============================================
// RESIZE
// ============================================

window.addEventListener("resize", () => {
  camera.aspect = window.innerWidth / window.innerHeight;
  camera.updateProjectionMatrix();
  renderer.setSize(window.innerWidth, window.innerHeight);
});

// ============================================
// EVENTOS TAURI
// ============================================

if (listen) {
  listen("avatar_state_change", (event) => {
    const newState = event.payload;
    if (typeof newState === "string") {
      avatarState = newState;
      // Traducir estado al español y aplicar clase CSS con color.
      const indicator = document.getElementById("state-indicator");
      const translations = {
        idle: "En espera",
        listening: "Escuchando",
        thinking: "Pensando",
        speaking: "Hablando",
      };
      indicator.textContent = translations[newState] || newState;
      // Actualizar clase CSS para el color.
      indicator.className = "state-" + newState;
      console.log("[avatar] Estado:", newState);
    }
  });
}

// ============================================
// CONTROL DE ROTACIÓN MANUAL
// ============================================

// Rueda del mouse: girar el modelo.
canvas.addEventListener(
  "wheel",
  (e) => {
    e.preventDefault();
    userRotationY += e.deltaY * 0.005;
  },
  { passive: false },
);

// ============================================
// INTERACCIÓN: clicks, drag, cerrar
// ============================================
//
// Click simple  → toggle dictado (inmediato, sin delay).
// 3 clicks rápidos (en <600ms) → cerrar avatar.
// Click derecho → toggle ventana de chat.
// Drag (>10px) → mover ventana.
// Rueda         → girar modelo.

let clickCount = 0;
let clickTimer = null;
let mouseDownPos = { x: 0, y: 0 };
let didDrag = false;
let mouseMoved = false;

canvas.addEventListener("mousedown", (e) => {
  if (e.button === 0) {
    // Botón izquierdo: preparar para click/drag.
    mouseDownPos = { x: e.screenX, y: e.screenY };
    didDrag = false;
    mouseMoved = false;
  } else if (e.button === 2) {
    // Botón derecho: toggle chat directamente desde mousedown.
    // No usar contextmenu porque WebKitGTK no lo dispara en ventanas
    // transparentes sin decoraciones.
    e.preventDefault();
    if (invoke) {
      invoke("toggle_chat_window").catch((err) =>
        console.error("[avatar] toggle chat:", err),
      );
    }
  }
});

canvas.addEventListener("mousemove", (e) => {
  if (e.buttons & 1) {
    const dx = e.screenX - mouseDownPos.x;
    const dy = e.screenY - mouseDownPos.y;
    const dist = Math.sqrt(dx * dx + dy * dy);
    if (dist > 5) {
      mouseMoved = true;
    }
    if (!didDrag && (Math.abs(dx) > 10 || Math.abs(dy) > 10)) {
      didDrag = true;
      invoke("start_dragging").catch(() => {});
      clickCount = 0;
      clearTimeout(clickTimer);
    }
  }
});

canvas.addEventListener("click", (e) => {
  e.preventDefault();
  if (!invoke || didDrag || mouseMoved) return;

  clickCount++;
  clearTimeout(clickTimer);

  if (clickCount >= 3) {
    // Triple-click: cerrar avatar.
    clickCount = 0;
    clearTimeout(clickTimer);
    invoke("close_avatar_window").catch(() => {});
    return;
  }

  // SOLO el primer click de una ráfaga dispara el toggle de dictado.
  // Los clicks 2 y 3 se absorben para detección de triple-click.
  // Esto previene la cascada de stt_start/stt_stop cuando el usuario
  // hace clicks rápidos: el primer click toggle, los siguientes no
  // hacen nada (se cuentan para triple-click).
  if (clickCount === 1) {
    if (avatarState === "listening") {
      invoke("stt_stop").catch(() => {});
    } else {
      invoke("stt_start", { modelId: null }).catch((err) => {
        console.warn("[avatar] stt_start rechazado:", err);
      });
    }
  }

  // Resetear contador después de 600ms sin clicks.
  clickTimer = setTimeout(() => {
    clickCount = 0;
  }, 600);
});

// Suprimir el menú contextual del navegador (click derecho).
// El toggle del chat se maneja en mousedown arriba.
window.addEventListener("contextmenu", (e) => {
  e.preventDefault();
});
