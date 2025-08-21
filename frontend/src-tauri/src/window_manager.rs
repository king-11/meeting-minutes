use tauri::{AppHandle, Manager, Runtime, Emitter};
use tauri_plugin_store::StoreExt;
use serde_json::json;
use serde::{Deserialize, Serialize};

#[tauri::command]
pub async fn show_floating_window<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("floating") {
        window.show().map_err(|e| e.to_string())?;
        window.set_always_on_top(true).map_err(|e| e.to_string())?;
    } else {
        return Err("Floating window not found".to_string());
    }
    Ok(())
}

#[tauri::command]
pub async fn hide_floating_window<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("floating") {
        window.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn save_window_position<R: Runtime>(
    app: AppHandle<R>, 
    x: i32, 
    y: i32
) -> Result<(), String> {
    let store = app.store("window_preferences.json")
        .map_err(|e| e.to_string())?;
    
    store.set("floating_window_position", json!({ "x": x, "y": y }));
    store.save().map_err(|e| e.to_string())?;
    
    Ok(())
}

#[tauri::command]
pub async fn get_window_position<R: Runtime>(app: AppHandle<R>) -> Result<(i32, i32), String> {
    let store = app.store("window_preferences.json")
        .map_err(|e| e.to_string())?;
    
    if let Some(position) = store.get("floating_window_position") {
        let x = position.get("x")
            .and_then(|v| v.as_i64())
            .unwrap_or(-1) as i32;
        let y = position.get("y")
            .and_then(|v| v.as_i64())
            .unwrap_or(-1) as i32;
        
        if x != -1 && y != -1 {
            return Ok((x, y));
        }
    }
    
    // Return default position (center of screen)
    if let Some(monitor) = app.primary_monitor().map_err(|e| e.to_string())? {
        let size = monitor.size();
        let x = (size.width as i32 - 220) / 2; // 220px is window width, center horizontally
        let y = (size.height as i32 - 90) / 2; // 90px is window height, center vertically
        Ok((x, y))
    } else {
        // Fallback to approximate center for common screen size
        Ok((850, 450)) // Approximate center for 1920x1080
    }
}

#[tauri::command]
pub async fn toggle_recording_with_ui_feedback<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    // Check if recording is active
    let is_recording = crate::is_recording();
    
    if !is_recording {
        // Start recording and show floating window
        // Start recording
        crate::start_recording(app.clone()).await?;
        show_floating_window(app.clone()).await?;
        app.emit("start-recording-from-tray", ()).map_err(|e| e.to_string())?;
    } else {
        // Stop recording
        let args = crate::RecordingArgs {
            save_path: String::new(),
        };
        let _result = crate::stop_recording(app.clone(), args).await?;
        app.emit("stop-recording-from-tray", ()).map_err(|e| e.to_string())?;
        
        // The floating window will auto-hide after showing confirmation
    }
    
    Ok(())
}