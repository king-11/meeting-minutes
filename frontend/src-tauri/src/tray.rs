use tauri::{
    AppHandle, Manager, Runtime, Emitter,
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
                                let _ = window.hide();
                            } else {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
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
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            } else {
                log::warn!("Could not find main window");
            }
        }
        "settings" => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
                let _ = window.eval("window.location.href = '/settings'");
            }
        }
        "quit" => {
            app.exit(0);
        }
        _ => {}
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