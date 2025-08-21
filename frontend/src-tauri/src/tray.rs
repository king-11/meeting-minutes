use tauri::{
    AppHandle, Manager, Runtime, Emitter, WebviewUrl, WebviewWindowBuilder,
    menu::{MenuBuilder, MenuItemBuilder, PredefinedMenuItem},
    tray::{TrayIconBuilder, TrayIconEvent, MouseButton},
};
use std::sync::atomic::{AtomicBool, Ordering};

static RECORDING_STATE: AtomicBool = AtomicBool::new(false);

pub fn create_tray<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let toggle_recording = MenuItemBuilder::with_id("toggle_recording", "Start Recording")
        .build(app)?;
    
    let open_window = MenuItemBuilder::with_id("open_window", "Open Main Window")
        .build(app)?;
    
    let settings = MenuItemBuilder::with_id("settings", "Settings")
        .build(app)?;
    
    let quit = MenuItemBuilder::with_id("quit", "Quit")
        .build(app)?;
    
    let separator = PredefinedMenuItem::separator(app)?;
    let separator2 = PredefinedMenuItem::separator(app)?;
    
    let menu = MenuBuilder::new(app)
        .item(&toggle_recording)
        .item(&separator)
        .item(&open_window)
        .item(&settings)
        .item(&separator2)
        .item(&quit)
        .build()?;
    
    let _tray = TrayIconBuilder::new()
        .menu(&menu)
        .tooltip("Meetily")
        .icon(app.default_window_icon().unwrap().clone())
        .on_menu_event(move |app, event| {
            handle_menu_event(app, event.id.as_ref());
        })
        .on_tray_icon_event(|tray, event| {
            match event {
                TrayIconEvent::Click { button, .. } => {
                    if button == MouseButton::Left {
                        if let Some(window) = tray.app_handle().get_webview_window("main") {
                            if window.is_visible().unwrap_or(false) {
                                if let Err(e) = window.hide() {
                                    log::error!("Failed to hide window on tray click: {}", e);
                                }
                            } else {
                                // Use the same activation logic as menu items
                                activate_and_show_window(tray.app_handle(), "main", None);
                            }
                        } else {
                            // Window doesn't exist, create and show it
                            log::info!("Main window doesn't exist, creating new one");
                            activate_and_show_window(tray.app_handle(), "main", None);
                        }
                    }
                }
                _ => {}
            }
        })
        .build(app)?;
    
    Ok(())
}

fn handle_menu_event<R: Runtime>(app: &AppHandle<R>, item_id: &str) {
    match item_id {
        "toggle_recording" => {
            let is_recording = RECORDING_STATE.load(Ordering::SeqCst);
            toggle_recording(app, !is_recording);
        }
        "open_window" => {
            activate_and_show_window(app, "main", None);
        }
        "settings" => {
            activate_and_show_window(app, "main", Some("window.location.href = '/settings'"));
        }
        "quit" => {
            app.exit(0);
        }
        _ => {}
    }
}

// Helper function to create main window if it doesn't exist
fn create_main_window<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    // Check if window already exists
    if app.get_webview_window("main").is_some() {
        return Ok(());
    }
    
    log::info!("Creating new main window");
    
    // Create a new main window with the same configuration as in tauri.conf.json
    let window = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
        .title("meetily")
        .inner_size(1200.0, 800.0)
        .resizable(true)
        .fullscreen(false)
        .decorations(true)
        .build()
        .map_err(|e| {
            log::error!("Failed to create main window: {}", e);
            e.to_string()
        })?;
    
    // Show the window
    if let Err(e) = window.show() {
        log::error!("Failed to show new main window: {}", e);
    }
    
    // Set focus to the window
    if let Err(e) = window.set_focus() {
        log::error!("Failed to set focus to new main window: {}", e);
    }
    
    Ok(())
}

// Helper function to properly activate app and show window
fn activate_and_show_window<R: Runtime>(app: &AppHandle<R>, window_label: &str, eval_script: Option<&str>) {
    // First, activate the app (bring to foreground on macOS)
    #[cfg(target_os = "macos")]
    {
        if let Err(e) = app.show() {
            log::error!("Failed to activate app: {}", e);
        }
    }
    
    // Check if window exists, if not and it's the main window, create it
    if app.get_webview_window(window_label).is_none() && window_label == "main" {
        if let Err(e) = create_main_window(app) {
            log::error!("Failed to create main window: {}", e);
            return;
        }
    }
    
    // Then show the window
    if let Some(window) = app.get_webview_window(window_label) {
        // Unminimize if minimized
        if let Ok(is_minimized) = window.is_minimized() {
            if is_minimized {
                if let Err(e) = window.unminimize() {
                    log::error!("Failed to unminimize window: {}", e);
                }
            }
        }
        
        // Show the window
        if let Err(e) = window.show() {
            log::error!("Failed to show window: {}", e);
        }
        
        // Set focus to the window
        if let Err(e) = window.set_focus() {
            log::error!("Failed to set focus: {}", e);
        }
        
        // Execute any eval script if provided
        if let Some(script) = eval_script {
            if let Err(e) = window.eval(script) {
                log::error!("Failed to execute script: {}", e);
            }
        }
    } else {
        log::warn!("Could not find window with label: {}", window_label);
    }
}

pub fn toggle_recording<R: Runtime>(app: &AppHandle<R>, start: bool) {
    RECORDING_STATE.store(start, Ordering::SeqCst);
    update_tray_menu(app, start);
    
    if start {
        let _ = app.emit("start-recording-from-tray", ());
    } else {
        let _ = app.emit("stop-recording-from-tray", ());
    }
}

pub fn update_tray_menu<R: Runtime>(app: &AppHandle<R>, recording: bool) {
    let label = if recording {
        "Stop Recording"
    } else {
        "Start Recording"
    };
    
    // Rebuild the menu with updated label
    let toggle_recording = MenuItemBuilder::with_id("toggle_recording", label)
        .build(app).ok();
    
    if toggle_recording.is_none() {
        return;
    }
    
    let open_window = MenuItemBuilder::with_id("open_window", "Open Main Window")
        .build(app).ok();
    
    let settings = MenuItemBuilder::with_id("settings", "Settings")
        .build(app).ok();
    
    let quit = MenuItemBuilder::with_id("quit", "Quit")
        .build(app).ok();
    
    if let (Some(toggle), Some(open), Some(settings_item), Some(quit_item)) = 
        (toggle_recording, open_window, settings, quit) {
        
        if let Ok(separator) = PredefinedMenuItem::separator(app) {
            if let Ok(separator2) = PredefinedMenuItem::separator(app) {
                if let Ok(menu) = MenuBuilder::new(app)
                    .item(&toggle)
                    .item(&separator)
                    .item(&open)
                    .item(&settings_item)
                    .item(&separator2)
                    .item(&quit_item)
                    .build() {
                    
                    // Update the tray menu
                    if let Some(tray) = app.tray_by_id("main") {
                        let _ = tray.set_menu(Some(menu));
                    }
                }
            }
        }
    }
}