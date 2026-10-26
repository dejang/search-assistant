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
use std::time::Duration;

use tauri::{Emitter, Manager};
use tauri_plugin_global_shortcut::{Code, Modifiers, ShortcutState};

use agent::Agent;
use agent::progress::ResearchProgress;
use agent::tools::Tooling;
use inference_engine::{Engine, EngineStatus, InferenceEngine, get_inference_engine};
use model_store::ModelStore;
use url_extractor::UrlExtractor;

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
pub fn run(page_budget: Duration) -> Result<(), tauri::Error> {
    let level = log_level();
    tauri::Builder::default()
        .plugin(
            // The plugin default caps the file at 40 KB and deletes it on rotation,
            // which would erase the trace before anyone opens it.
            tauri_plugin_log::Builder::new()
                .level(level)
                .max_file_size(4_000_000)
                .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepSome(5))
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
        .setup(move |app| {
            use tauri_plugin_global_shortcut::GlobalShortcutExt;
            log::info!("setup: begin; page budget {page_budget:?}, log level {level:?}");
            app.global_shortcut().register("alt+shift+space")?;
            tray_icon::mount(app.handle())?;
            app.manage(Arc::new(UrlExtractor::default().with_timeout(page_budget)));
            let progress_handle = app.handle().clone();
            app.manage(Arc::new(ResearchProgress::new().on_activity(
                move |activity| {
                    let _ = progress_handle.emit("research://activity", activity);
                },
            )));
            ModelStore::start(app.handle(), "Qwen", "Qwen3-8B-GGUF", "Q4_K_M");
            start_engine(app.handle());
            log::info!("setup: done; download and engine boot are in flight");
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
                log::info!("exit: shutting down the inference engine");
                engine.shutdown();
            }
        });
    Ok(())
}

pub(crate) fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let was_visible = window.is_visible().unwrap_or(false);
        log::debug!("window: showing main (was_visible={was_visible})");
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
            log::debug!("window: hiding main");
            let _ = window.hide();
        } else {
            show_main_window(app);
        }
    } else {
        log::warn!("window: no main window to toggle");
    }
}

/// The log filter `RESEARCH_BUDDY_LOG` asks for; `Info` when it is unset or
/// spells a level the app does not recognize.
fn log_level() -> log::LevelFilter {
    match std::env::var("RESEARCH_BUDDY_LOG").ok().as_deref() {
        Some("trace") => log::LevelFilter::Trace,
        Some("debug") => log::LevelFilter::Debug,
        Some("warn") => log::LevelFilter::Warn,
        Some("error") => log::LevelFilter::Error,
        _ => log::LevelFilter::Info,
    }
}

/// `text` within `max_chars` characters, with the rest counted rather than
/// pasted, so one log line stays one line.
#[must_use]
pub(crate) fn clipped(text: &str, max_chars: usize) -> String {
    let chars = text.chars().count();
    if chars <= max_chars {
        return text.to_string();
    }
    let rest = chars - max_chars;
    let head: String = text.chars().take(max_chars).collect();
    format!("{head}… (+{rest} chars)")
}

/// Boot the engine, then the agent, once the model has settled on disk.
fn start_engine(app: &tauri::AppHandle) {
    let handle = app.clone();
    drop(tauri::async_runtime::spawn(async move {
        log::info!("engine: boot started");
        if let Some(store) = handle.try_state::<Arc<ModelStore>>() {
            let mut waited = 0_u32;
            while store.status().phase == model_store::DownloadPhase::Downloading {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                waited += 1;
                if waited.is_multiple_of(50) {
                    log::info!("engine: still waiting on the model download");
                }
            }
            log::info!("engine: download loop left at {:?}", store.status().phase);
        } else {
            log::warn!("engine: no model store is mounted");
        }
        let booted = std::time::Instant::now();
        match get_inference_engine(&handle).await {
            Ok(engine) => {
                log::info!(
                    "engine: ready at {} in {:?}",
                    engine.endpoint(),
                    booted.elapsed()
                );
                let extractor = Arc::clone(&handle.state::<Arc<UrlExtractor>>());
                let progress = Arc::clone(&handle.state::<Arc<ResearchProgress>>());
                let agent = Agent::new(engine.endpoint(), "qwen3-8b")
                    .with_tooling(Tooling::new(extractor, progress, 8_000));
                handle.manage(Arc::new(engine));
                handle.manage(Arc::new(agent));
                log::info!("agent: lead and subagent tooling attached");
                let _ = handle.emit("engine://status", EngineStatus::Online);
            }
            Err(err) => {
                log::error!("engine: boot failed: {err}");
                handle.manage(EngineStatus::Offline);
                let _ = handle.emit("engine://status", EngineStatus::Offline);
            }
        }
    }));
}

#[cfg(test)]
mod tests {
    use super::clipped;

    #[test]
    fn text_within_the_limit_is_passed_through() {
        assert_eq!(clipped("rust borrow checker", 120), "rust borrow checker");
    }

    #[test]
    fn longer_text_keeps_its_head_and_counts_the_rest() {
        let long = "a".repeat(200);
        let cut = clipped(&long, 10);
        assert!(cut.starts_with("aaaaaaaaaa\u{2026}"));
        assert!(cut.contains("(+190 chars)"));
    }

    #[test]
    fn clipping_never_splits_a_multibyte_codepoint() {
        let text = "a\u{e9}\u{1f980}b".repeat(50);
        let cut = clipped(&text, 3);
        assert!(cut.starts_with("a\u{e9}\u{1f980}"));
        assert!(cut.contains("(+197 chars)"));
    }
}
