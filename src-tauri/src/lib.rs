// hf-hub's download future nests `Box::pin` and opaque builders deeply enough
// that proving it `Send` overflows rustc's default recursion budget. Raising
// the budget lets that check finish instead of failing the build.
#![recursion_limit = "512"]

pub mod agent;
pub mod model_store;
pub mod url_extractor;

use tauri::{Emitter, Manager};
use tauri_plugin_global_shortcut::{Code, Modifiers, ShortcutState};

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

#[tauri::command]
fn hide_window(app: tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
    drop(app);
}

/// Start the application: register global shortcuts, create windows, run the event loop.
///
/// # Errors
///
/// Returns [`tauri::Error`] if the runtime or a plugin fails to initialize.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() -> Result<(), tauri::Error> {
    tauri::Builder::default()
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
            ModelStore::start(app.handle(), "Qwen", "Qwen3-8B-GGUF", "Q4_K_M");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            hide_window,
            model_store::model_download,
            model_store::model_download_status
        ])
        .run(tauri::generate_context!())
}
