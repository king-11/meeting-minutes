use anyhow::Result;
use tauri::{AppHandle, Emitter, Runtime};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use log::{info as log_info, error as log_error};

const RECORDING_SHORTCUT: &str = "Alt+Space";

/// Register the global shortcut for recording
pub fn register_recording_shortcut<R: Runtime>(app: &AppHandle<R>) -> Result<()> {
    // Parse the shortcut string
    let shortcut: Shortcut = RECORDING_SHORTCUT.parse()?;
    
    let app_handle = app.clone();
    
    app.global_shortcut().on_shortcut(shortcut.clone(), move |_app, _shortcut, event| {
        if event.state == ShortcutState::Pressed {
            log_info!("Global shortcut triggered: {}", RECORDING_SHORTCUT);
            
            // Emit event to toggle recording
            if let Err(e) = app_handle.emit("toggle-recording-shortcut", ()) {
                log_error!("Failed to emit toggle-recording event: {}", e);
            }
        }
    })?;
    
    log_info!("Registered global shortcut: {}", RECORDING_SHORTCUT);
    Ok(())
}

/// Unregister all global shortcuts
pub fn unregister_all_shortcuts<R: Runtime>(app: &AppHandle<R>) -> Result<()> {
    app.global_shortcut().unregister_all()?;
    log_info!("Unregistered all global shortcuts");
    Ok(())
}

/// Check if a shortcut is registered
pub fn is_shortcut_registered<R: Runtime>(app: &AppHandle<R>, shortcut: &str) -> bool {
    if let Ok(shortcut) = shortcut.parse::<Shortcut>() {
        app.global_shortcut().is_registered(shortcut)
    } else {
        false
    }
}