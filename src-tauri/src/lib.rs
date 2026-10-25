// hf-hub's download future nests `Box::pin` and opaque builders deeply enough
// that proving it `Send` overflows rustc's default recursion budget. Raising
// the budget lets that check finish instead of failing the build.
#![recursion_limit = "512"]

pub mod agent;
pub mod local_engine;
pub mod model_store;
mod tray_icon;
pub mod url_extractor;

use std::sync::Arc;
use std::time::Duration;

use tauri::{Emitter, Manager};
use tauri_plugin_global_shortcut::{Code, Modifiers, ShortcutState};

use agent::BuddyAgent;
use agent::RunBudget;
use agent::progress::ResearchProgress;
use agent::tools::Tooling;
use local_engine::{Budget, EngineClient, LocalEngine, Sampling, ServerConfig};
use model_store::ModelStore;
use url_extractor::UrlExtractor;

pub(crate) fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let was_visible = window.is_visible().unwrap_or(false);
        let _ = window.show();
        let _ = window.set_focus();
        if !was_visible {
            let _ = window.emit("window-open", ());
        }
    }
}

pub(crate) fn toggle_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
        } else {
            show_main_window(app);
        }
    }
}

fn mount_agent(app: &tauri::AppHandle) {
    let engine = Arc::clone(&app.state::<Arc<LocalEngine>>());
    let extractor = Arc::clone(&app.state::<Arc<UrlExtractor>>());
    let progress = Arc::clone(&app.state::<Arc<ResearchProgress>>());
    let agent = BuddyAgent::with_research(
        EngineClient::new(engine),
        "qwen3-8b",
        Tooling::new(extractor, progress, 8_000),
        RunBudget {
            turns: 12,
            retries: 3,
            tools: 4,
        },
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

/// Grows the window into the research panel, or shrinks it back to the bar.
#[tauri::command]
fn set_research_view(app: tauri::AppHandle, expanded: bool) {
    let (width, height) = if expanded { (980, 680) } else { (760, 160) };
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_size(tauri::LogicalSize::new(width, height));
        let _ = window.center();
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
            tray_icon::mount(app.handle())?;
            app.manage(Arc::new(UrlExtractor::default()));
            app.manage(Arc::new(ResearchProgress::new(app.handle().clone())));
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
                ServerConfig {
                    binary: std::env::current_exe()?.with_file_name("llama-server"),
                    slots: 4,
                    slot_context: 16_384,
                    cache_mib: 2_048,
                    sampling: Sampling {
                        temperature: 0.7,
                        top_p: 0.8,
                        top_k: 20,
                        min_p: 0.0,
                    },
                    log: app
                        .path()
                        .app_log_dir()
                        .ok()
                        .map(|dir| dir.join("llama-server.log")),
                },
            );
            mount_agent(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            hide_window,
            set_research_view,
            agent::research_ask,
            local_engine::engine_status,
            model_store::model_download,
            model_store::model_download_status
        ])
        .build(tauri::generate_context!())?
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event
                && let Some(engine) = app.try_state::<Arc<LocalEngine>>()
            {
                engine.shutdown();
            }
        });
    Ok(())
}
