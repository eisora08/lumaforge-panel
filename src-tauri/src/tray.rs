use tauri::menu::{MenuBuilder, MenuItemBuilder, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{App, Manager};

/// Fallback 16x16 RGBA block used when the app icon is unavailable.
const TRAY_ICON_RGBA: [u8; 16 * 16 * 4] = {
    let mut data = [0u8; 16 * 16 * 4];
    let mut i = 0;
    while i < 16 * 16 * 4 {
        data[i] = 0x18; // B
        data[i + 1] = 0x2a; // G
        data[i + 2] = 0x6e; // R
        data[i + 3] = 0xFF; // A
        i += 4;
    }
    data
};

pub fn init_system_tray(app: &mut App) -> tauri::Result<()> {
    let handle = app.handle();

    let icon = match app.default_window_icon() {
        Some(icon) => icon.clone(),
        None => tauri::image::Image::new_owned(TRAY_ICON_RGBA.to_vec(), 16, 16),
    };

    let open_item = MenuItemBuilder::with_id("open", "Open LumaForge Panel").build(handle)?;
    let quit_item = MenuItemBuilder::with_id("quit", "Quit").build(handle)?;
    let separator = PredefinedMenuItem::separator(handle)?;

    let menu = MenuBuilder::new(handle)
        .item(&open_item)
        .item(&separator)
        .item(&quit_item)
        .build()?;

    TrayIconBuilder::new()
        .icon(icon)
        .tooltip("LumaForge Panel")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app_handle, event| match event.id.as_ref() {
            "quit" => {
                app_handle.exit(0);
            }
            "open" => {
                if let Some(window) = app_handle.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.unminimize();
                    let _ = window.set_focus();
                }
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    let visible = window.is_visible().unwrap_or(false);
                    if visible {
                        let _ = window.hide();
                    } else {
                        let _ = window.show();
                        let _ = window.unminimize();
                        let _ = window.set_focus();
                    }
                }
            }
        })
        .build(app)?;

    Ok(())
}
