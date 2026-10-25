// hf-hub's download future nests `Box::pin` and opaque builders deeply enough
// that proving it `Send` overflows rustc's default recursion budget. Raising
// the budget lets that check finish instead of failing the build.
#![recursion_limit = "512"]

pub mod agent;
pub mod inference_engine;
pub mod model_store;
mod tray_icon;
pub mod url_extractor;

use std::sync::Arc;

use tauri::{Emitter, Manager};
use tauri_plugin_global_shortcut::{Code, Modifiers, ShortcutState};

use agent::BuddyAgent;
use agent::RunBudget;
use agent::progress::ResearchProgress;
use agent::tools::Tooling;
use inference_engine::{Engine, EngineStatus, InferenceEngine, get_inference_engine};
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

fn start_engine(app: &tauri::AppHandle) {
    let handle = app.clone();
    drop(tauri::async_runtime::spawn(async move {
        if let Some(store) = handle.try_state::<Arc<ModelStore>>() {
            while store.status().phase == model_store::DownloadPhase::Downloading {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }
        match get_inference_engine(&handle).await {
            Ok(engine) => {
                let extractor = Arc::clone(&handle.state::<Arc<UrlExtractor>>());
                let progress = Arc::clone(&handle.state::<Arc<ResearchProgress>>());
                let agent = BuddyAgent::with_research(
                    engine.endpoint(),
                    "qwen3-8b",
                    Tooling::new(extractor, progress, 8_000),
                    RunBudget {
                        turns: 12,
                        retries: 3,
                        tools: 4,
                    },
                );
                handle.manage(Arc::new(engine));
                handle.manage(Arc::new(agent));
                let _ = handle.emit("engine://status", EngineStatus::Online);
            }
            Err(err) => {
                log::error!("Failed to initialize inference engine: {err}");
                handle.manage(EngineStatus::Offline);
                let _ = handle.emit("engine://status", EngineStatus::Offline);
            }
        }
    }));
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
            start_engine(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            hide_window,
            set_research_view,
            agent::research_ask,
            inference_engine::engine_status,
            model_store::model_download,
            model_store::model_download_status
        ])
        .build(tauri::generate_context!())?
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event
                && let Some(engine) = app.try_state::<Arc<InferenceEngine>>()
            {
                engine.shutdown();
            }
        });
    Ok(())
}
