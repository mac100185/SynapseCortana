Proyecto Synapse-Cortana
1. Nombre del Proyecto
Proyecto Synapse-Cortana
(Haciendo alusión a la "sinapsis", el puente de conexión biológica y digital que unía a Cortana con el sistema nervioso de John-117 en la serie, y que aquí une tu escritorio con la IA).
2. Contexto Actual (2026)
En el panorama tecnológico actual, contamos con motores cognitivos locales hiperpotentes (Ollama) y frameworks de automatización y agencia de código abierto (OpenClaw). Sin embargo, la interacción humana con estos sistemas sigue estando fragmentada y deshumanizada, limitada a terminales de texto frío o interfaces de voz incorpóreas. La IA es tratada exclusivamente como un artefacto utilitario.
Existe la necesidad tecnológica y filosófica de evolucionar hacia una coexistencia empática. Para ello, se requiere un canal de comunicación visual y auditivo que dote a la IA de presencia, identidad y gesticulación emocional, inspirándose en la naturaleza simbiótica de Cortana en la Silver Timeline de Halo.
3. Objetivo General
Desarrollar un canal de interfaz visual y auditivo (Frontend) interactivo y tridimensional en tiempo real, modelado a partir del personaje de Cortana (Paramount+), que se integre de forma nativa como un nodo cliente en el ecosistema actual de OpenClaw/Ollama, humanizando la interacción hombre-máquina mediante un avatar holográfico dinámico y clonación de voz local.
4. Etapas de Desarrollo (Roadmap)
Para construir el "Punto 3" sin alterar lo que ya tienes funcionando, dividiremos el desarrollo en 4 fases incrementales:
FASE 1: El Cascarón Conectivo (Infraestructura del Canal)
    • Meta: Lograr que una ventana de software externa reciba y envíe datos a tu servidor de OpenClaw.
    • Tareas:
        ◦ Establecer la conexión mediante WebSockets o API REST entre el nuevo cliente (Frontend) y OpenClaw.
        ◦ Validar el flujo de datos: Enviar un texto/audio de prueba desde el cliente y recibir la respuesta procesada por Ollama de vuelta en la interfaz.
FASE 2: La Voz de Cortana (Módulo Auditivo Local)
    • Meta: Que la respuesta de texto fría de Ollama se convierta instantáneamente en la voz de Cortana.
    • Tareas:
        ◦ Integrar un motor de Texto a Voz (TTS) local (como XTTS v2) dentro del pipeline del canal.
        ◦ Entrenar/Clonar el modelo de voz utilizando muestras de audio de la actriz de voz de Cortana de la serie.
        ◦ Optimizar la latencia para que el renderizado de audio ocurra en milisegundos tras recibir la respuesta de OpenClaw.
FASE 3: El Cuerpo Holográfico (Módulo Visual 3D)
    • Meta: Darle presencia física en el escritorio mediante un avatar animado y translúcido.
    • Tareas:
        ◦ Configurar el entorno gráfico (usando herramientas como Godot Engine o Electron/WebGL) con soporte de ventana transparente (sin bordes).
        ◦ Implementar el modelo 3D de Cortana con sombreadores (shaders) que simulen emisión de luz azul y estética digital/holográfica.
        ◦ Diseñar las animaciones base: Reposo (Idle), Escuchando (brillo más intenso) y Pensando (líneas de código moviéndose rápido).
FASE 4: La Sinergia Empática (Sincronización y Expresión)
    • Meta: Unir la voz con el cuerpo para que Cortana "sienta" y gesticule.
    • Tareas:
        ◦ Implementar el sistema de Lip-Sync automatizado para que la boca del avatar se mueva al ritmo del audio generado en la Fase 2.
        ◦ Programar un analizador de texto básico que detecte la intención emocional de la respuesta de Ollama y dispare expresiones o poses acordes (ej. una sonrisa irónica si la respuesta es sarcástica).
1. Requisitos de Hardware para "Synapse-Cortana" (El Canal Frontend)
Para asegurar que funcione tanto en una laptop de oficina básica como en una estación de trabajo premium, dividiremos el consumo del frontend en dos perfiles técnicos según la capacidad del equipo:
Perfil Básico (Para ordenadores limitados / CPUs antiguas)
    • Ajuste del sistema: El avatar 3D reduce sus efectos visuales (sin reflejos complejos) y el motor de voz cambia automáticamente a un modelo optimizado para CPU (como Piper o Kokoro en lugar de XTTS v2).
    • Procesador (CPU): Cualquier procesador Dual-Core (Intel i3 / AMD Ryzen 3 / Apple M1) de los últimos 6 años.
    • Memoria RAM: ~200 MB a 400 MB dedicados exclusivamente a la interfaz de Cortana.
    • Gráficos (GPU): Gráficos integrados (Intel UHD, AMD Radeon Vega).
    • Almacenamiento: ~150 MB para el programa base y un modelo de voz ultraligero.
Perfil Premium (Para PCs con tarjeta gráfica / Apple Silicon)
    • Ajuste del sistema: Avatar 3D con shaders holográficos avanzados emisivos de luz en tiempo real, física en el cabello y clonación de voz idéntica de alta fidelidad vía GPU (XTTS v2).
    • Procesador (CPU): Quad-Core o superior.
    • Memoria RAM: ~1 GB dedicado (para almacenar en caché buffers de audio de alta calidad).
    • Gráficos (GPU): Dedicada (NVIDIA GTX 1060 / RTX o superior) o GPU integrada en Apple Silicon (M1/M2/M3). Requiere al menos 2 GB de VRAM libre.
    • Almacenamiento: ~2.5 GB (el grueso se lo lleva el modelo de clonación de voz de alta fidelidad).
💡 Nota sobre el ecosistema completo: Si el usuario decide correr también Ollama y OpenClaw de forma local en el mismo ordenador básico, el sistema general requerirá un mínimo absoluto de 8 GB de RAM globales para mover un modelo de lenguaje pequeño (como Llama-3-8B cuantizado o Phi-3).
2. Decisiones de Desarrollo para Multiplataforma y Portabilidad
Para cumplir con tu requisito de instalación/desinstalación limpia y soporte completo para Windows, Linux y macOS, la mejor decisión de ingeniería es descartar instaladores intrusivos en el sistema y optar por aplicaciones portables autocontenidas.
Descartando motores pesados, la tecnología ideal para esta interfaz es Godot Engine 4.x compilada en modo portátil o un entorno web nativo ligero como Tauri (una alternativa a Electron que usa el motor web nativo del sistema operativo, reduciendo el tamaño del ejecutable de 150MB a solo 10MB y consumiendo poquísima RAM).
Estrategia de Portabilidad por Sistema Operativo:
    • En Windows: El proyecto se distribuye como un único archivo .exe acompañado de una carpeta de recursos (o todo empaquetado en un .zip). Para desinstalar, el usuario simplemente borra la carpeta. No se escribe nada en el Registro de Windows ni en Program Files.
    • En macOS: Se compila como un archivo .app autocontenido. El usuario lo arrastra a su carpeta de Aplicaciones para "instalarlo" y lo arrastra a la Papelera para borrarlo por completo. Compatible nativamente con arquitecturas Intel y Apple Silicon (M1/M2/M3).
    • En Linux: Se empaqueta bajo el formato AppImage. Un AppImage es un archivo único ejecutable que contiene todas las dependencias que el programa necesita para correr. Funciona en Ubuntu, Fedora, Arch, Debian, etc., sin necesidad de usar apt, pacman ni privilegios de sudo. Borrar el archivo significa desinstalar el programa por completo.
3. Almacenamiento de Configuraciones Limpias
Para evitar dejar "basura" en el ordenador tras borrar a Cortana, todas las configuraciones locales (la dirección IP/Puerto de tu OpenClaw, las preferencias del usuario o el volumen) se guardarán en un único archivo de texto plano (config.json) ubicado en la misma carpeta raíz del ejecutable (Modo Portable). Si eliminas la carpeta del proyecto, el sistema queda 100% limpio.
Con estas bases claras, garantizamos que Synapse-Cortana sea un software ágil, respetuoso con los recursos del usuario y sumamente fácil de distribuir.
