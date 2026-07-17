// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;

fn main() {
    // Inicializar logging. Respeta RUST_LOG (default: "info").
    // Ejemplos:
    //   RUST_LOG=debug ./synapse-cortana         # todo debug
    //   RUST_LOG=synapse_cortana=trace ...         # solo nuestro crate
    //   RUST_LOG=warn ./synapse-cortana           # solo warnings+
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    // Silenciar los warnings informativos de GStreamer/WebKit2GTK.
    if std::env::var_os("GST_DEBUG").is_none() {
        std::env::set_var("GST_DEBUG", "0");
    }
    if std::env::var_os("G_MESSAGES_DEBUG").is_none() {
        std::env::set_var("G_MESSAGES_DEBUG", "none");
    }

    // FASE 3 distribución: configurar GST_PLUGIN_PATH ANTES de que
    // Tauri/WebKitGTK inicialice GStreamer. Si lo hacemos en setup(),
    // es demasiado tarde: WebKitWebProcess ya se ha iniciado sin
    // GST_PLUGIN_PATH y no encuentra los plugins del bundle.
    setup_gstreamer_plugins();

    // FASE 3 distribución: inicializar el directorio de recursos del
    // bundle ANTES de que AppState::default() arranque la pre-carga de
    // voz en background. Si no lo hacemos aquí, BUNDLE_RESOURCE_DIR
    // es None cuando la pre-carga intenta copiar la voz desde el bundle,
    // y descarga 114MB de internet.
    setup_bundle_resources();

    // Modo CLI para testing sin GUI: `synapse-cortana --cli-test-handshake [URL]`
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--cli-test-handshake") {
        let url = args
            .iter()
            .position(|a| a == "--url")
            .and_then(|i| args.get(i + 1))
            .cloned()
            .unwrap_or_else(|| "http://127.0.0.1:18789".to_string());
        let token = args
            .iter()
            .position(|a| a == "--token")
            .and_then(|i| args.get(i + 1))
            .cloned();
        std::process::exit(synapse_cortana::cli_test_handshake(&url, token.as_deref()));
    }
    // Modo CLI para TTS sin GUI ni gateway:
    if args.iter().any(|a| a == "--cli-test-speak") {
        let voice = args
            .iter()
            .position(|a| a == "--voice")
            .and_then(|i| args.get(i + 1))
            .map(|s| s.as_str());
        let text = args
            .iter()
            .position(|a| a == "--text")
            .and_then(|i| args.get(i + 1))
            .map(|s| s.as_str());
        let out = args
            .iter()
            .position(|a| a == "--out")
            .and_then(|i| args.get(i + 1))
            .map(|s| s.as_str());
        std::process::exit(synapse_cortana::cli_test_speak(voice, text, out));
    }
    synapse_cortana::run();
}

/// Configura GST_PLUGIN_PATH antes de que Tauri/WebKitGTK arranque.
///
/// Busca los plugins de GStreamer en:
/// 1. ~/.config/synapse-cortana/gstreamer-plugins/ (ya extraídos)
/// 2. Si no existen, busca el .tar en el resource_dir del AppImage/DEB
///    y los extrae a ~/.config/.
///
/// Debe llamarse ANTES de `synapse_cortana::run()` porque WebKitGTK
/// inicializa GStreamer al crear la primera ventana, y el
/// WebKitWebProcess hereda las variables de entorno del proceso padre.
fn setup_gstreamer_plugins() {
    // 1. Verificar si ya están extraídos en ~/.config/.
    let config_dir = directories::ProjectDirs::from("ai", "openclaw", "synapse-cortana")
        .map(|d| d.config_dir().to_path_buf());
    let gst_target = config_dir
        .as_ref()
        .map(|c| c.join("gstreamer-plugins"))
        .unwrap_or_else(|| PathBuf::from("/tmp/synapse-gst-plugins"));

    if gst_target.exists() && gst_target.join("libgstapp.so").exists() {
        // Ya están extraídos. Configurar GST_PLUGIN_PATH.
        std::env::set_var("GST_PLUGIN_PATH", &gst_target);
        let scanner = gst_target.join("gst-plugin-scanner");
        if scanner.exists() {
            std::env::set_var("GST_PLUGIN_SCANNER", scanner);
        }
        log::info!(
            "[bundle] GST_PLUGIN_PATH: {} (ya extraído)",
            gst_target.display()
        );
        return;
    }

    // 2. Buscar el .tar en el resource_dir del bundle.
    // En un AppImage, el binario está en /tmp/.mount_XXX/usr/bin/
    // y los recursos en /tmp/.mount_XXX/usr/lib/<productName>/resources/
    // Detectamos el resource_dir buscando relativo al ejecutable.
    let exe = match std::env::current_exe() {
        Ok(e) => e,
        Err(_) => return,
    };

    // En AppImage: exe está en .../usr/bin/synapse-cortana
    // resources en .../usr/lib/Synapse Cortana/resources/
    let candidates = [
        exe.parent().and_then(|p| p.parent()).map(|p| {
            p.join("lib")
                .join("Synapse Cortana")
                .join("resources")
                .join("gstreamer-plugins.tar")
        }),
        // En desarrollo: exe está en target/release/
        exe.parent()
            .map(|p| p.join("resources").join("gstreamer-plugins.tar")),
    ];

    for gst_tar in candidates.iter().flatten() {
        if gst_tar.exists() {
            log::info!(
                "[bundle] extrayendo GStreamer plugins desde {}",
                gst_tar.display()
            );

            // Crear el directorio destino.
            if let Some(parent) = gst_target.parent() {
                let _ = std::fs::create_dir_all(parent);
            }

            // Extraer el tar.
            let status = std::process::Command::new("tar")
                .arg("xf")
                .arg(gst_tar)
                .arg("-C")
                .arg(gst_target.parent().unwrap_or(&PathBuf::from("/tmp")))
                .status();

            if let Ok(s) = status {
                if s.success() && gst_target.exists() {
                    std::env::set_var("GST_PLUGIN_PATH", &gst_target);
                    let scanner = gst_target.join("gst-plugin-scanner");
                    if scanner.exists() {
                        std::env::set_var("GST_PLUGIN_SCANNER", scanner);
                    }
                    log::info!(
                        "[bundle] GST_PLUGIN_PATH: {} (recién extraído)",
                        gst_target.display()
                    );
                    return;
                }
            }
            log::warn!("[bundle] fallo extrayendo GStreamer plugins");
            break;
        }
    }

    log::warn!("[bundle] GStreamer plugins no encontrados en el bundle");
}

/// Inicializa el directorio de recursos del bundle para que tts.rs y
/// stt.rs puedan copiar modelos desde el bundle en vez de descargarlos
/// de internet. Debe llamarse ANTES de `synapse_cortana::run()` porque
/// `AppState::default()` arranca la pre-carga de voz en background y
/// necesita `BUNDLE_RESOURCE_DIR` ya inicializado.
fn setup_bundle_resources() {
    let exe = match std::env::current_exe() {
        Ok(e) => e,
        Err(_) => return,
    };

    // En AppImage: exe está en .../usr/bin/synapse-cortana
    // resources en .../usr/lib/Synapse Cortana/
    // En desarrollo: exe está en target/release/
    // resources en src-tauri/resources/
    let candidates = [
        exe.parent()
            .and_then(|p| p.parent())
            .map(|p| p.join("lib").join("Synapse Cortana")),
        exe.parent().map(|p| {
            p.join("..")
                .join("src-tauri")
                .join("resources")
                .canonicalize()
                .unwrap_or_else(|_| p.join("resources"))
        }),
    ];

    for rd in candidates.iter().flatten() {
        if rd.join("resources").join("voices").exists() || rd.join("voices").exists() {
            log::info!("[bundle] resource_dir detectado: {}", rd.display());
            synapse_cortana::tts::init_bundle_resources(rd.clone());
            synapse_cortana::stt::init_bundle_resources(rd.clone());
            return;
        }
    }

    // Si no encontramos el resource_dir, inicializar con PathBuf vacío
    // (fallback a descarga de internet).
    log::warn!("[bundle] resource_dir no detectado, fallback a internet");
    synapse_cortana::tts::init_bundle_resources(PathBuf::new());
    synapse_cortana::stt::init_bundle_resources(PathBuf::new());
}
