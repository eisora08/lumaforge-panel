mod github;
mod panel;
mod paths;
mod postinstall;
mod settings;
mod state;
mod steam;
mod tools;
mod tray;

pub fn run() {
    use tauri::Manager;

    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            if let Err(error) = tray::init_system_tray(app) {
                eprintln!("[PANEL] Failed to create tray icon: {error}");
            }

            let show_on_start = !settings::load().startup.start_minimized;
            if let Some(window) = app.get_webview_window("main") {
                if show_on_start {
                    if let Err(error) = window.show() {
                        eprintln!("[PANEL] Failed to show window: {error}");
                    }
                } else if let Err(error) = window.hide() {
                    eprintln!("[PANEL] Failed to hide window: {error}");
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            panel::panel_status,
            panel::check_updates,
            panel::cdp_set_enabled,
            panel::set_tool_enabled,
            panel::install_runtime,
            panel::install_tool,
            panel::get_steam_status,
            panel::start_steam,
            panel::restart_steam,
            panel::get_app_version,
            panel::get_settings,
            panel::update_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running lumaforge-panel");
}
