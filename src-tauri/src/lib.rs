// hf-hub's download future nests `Box::pin` and opaque builders deeply enough
// that proving it `Send` overflows rustc's default recursion budget. Raising
// the budget lets that check finish instead of failing the build.
#![recursion_limit = "512"]

pub mod agent;
pub mod local_engine;
pub mod model_store;
pub mod url_extractor;

use std::sync::Arc;
use std::time::Duration;

use tauri::{Emitter, Manager};
use tauri_plugin_global_shortcut::{Code, Modifiers, ShortcutState};

use agent::BuddyAgent;
use local_engine::{Budget, EngineClient, LocalEngine};
use model_store::ModelStore;

fn toggle_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
        } else {
            let _ = window.show();
            let _ = window.set_focus();
            let _ = window.emit("window-open", ());
        }
    }
}

fn mount_agent(app: &tauri::AppHandle) {
    let engine = Arc::clone(&app.state::<Arc<LocalEngine>>());
    let agent = BuddyAgent::with_client(
        EngineClient::new(engine),
        "qwen3-8b",
        "You are a concise research assistant. Answer what was asked, in plain language.",
    );
    app.manage(Arc::new(agent));
}

#[tauri::command]
fn hide_window(app: tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
    drop(app);
}

/// Returns [`tauri::Error`] if the runtime or a plugin fails to initialize.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
/// # Errors
/// `tauri::Error` if the runtime or a plugin fails to initialize.
pub fn run() -> Result<(), tauri::Error> {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state == ShortcutState::Pressed
                        && shortcut.matches(Modifiers::ALT | Modifiers::SHIFT, Code::Space)
                    {
                        toggle_main_window(app);
                    }
                })
                .build(),
        )
        .setup(|app| {
            use tauri_plugin_global_shortcut::GlobalShortcutExt;
            app.global_shortcut().register("alt+shift+space")?;
            app.manage(Arc::new(url_extractor::UrlExtractor::new(8)));
            ModelStore::start(app.handle(), "Qwen", "Qwen3-8B-GGUF", "Q4_K_M");
            let store = app
                .handle()
                .try_state::<Arc<ModelStore>>()
                .map(|store| Arc::clone(&store));
            LocalEngine::start(
                app.handle(),
                store,
                Budget {
                    idle: Duration::from_secs(120),
                    load: Duration::from_secs(300),
                    answer: Duration::from_secs(120),
                },
                4,
            );
            mount_agent(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            hide_window,
            agent::chat_ask,
            local_engine::engine_status,
            model_store::model_download,
            model_store::model_download_status,
            url_extractor::search_web
        ])
        .run(tauri::generate_context!())
}
