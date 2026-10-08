mod github;
mod i18n;
mod panel;
mod paths;
mod postinstall;
mod settings;
mod state;
mod steam;
mod themes;
mod tools;
mod tray;

pub fn run() {
    use tauri::Manager;

    i18n::init_from_settings(&settings::load().language);

    let app = tauri::Builder::default()
        // Must be the first plugin: a second launch exits here and hands the
        // open/close signal to the running instance.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            if let Err(error) = tray::init_system_tray(app) {
                eprintln!("[PANEL] Failed to create tray icon: {error}");
            }

            let settings = settings::load();
            if let Some(window) = app.get_webview_window("main") {
                restore_geometry(app, &window, settings.window.as_ref());
                if settings.startup.start_minimized {
                    if let Err(error) = window.hide() {
                        eprintln!("[PANEL] Failed to hide window: {error}");
                    }
                } else if let Err(error) = window.show() {
                    eprintln!("[PANEL] Failed to show window: {error}");
                }
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            settings::capture_geometry(window, event);
        })
        .invoke_handler(tauri::generate_handler![
            panel::panel_status,
            panel::check_updates,
            panel::cdp_set_enabled,
            panel::set_tool_enabled,
            panel::install_runtime,
            panel::install_tool,
            panel::uninstall_tool,
            panel::get_steam_status,
            panel::start_steam,
            panel::restart_steam,
            panel::get_app_version,
            panel::open_app_data_dir,
            panel::get_settings,
            panel::update_settings,
            themes::themes_get,
            themes::theme_activate,
            themes::theme_deactivate,
            themes::theme_set_condition,
            themes::theme_reset_condition,
        ])
        .build(tauri::generate_context!())
        .expect("error while building lumaforge-panel");

    app.run(|_app, event| match event {
        tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit => {
            settings::flush_geometry();
        }
        _ => {}
    });
}

fn restore_geometry(
    app: &tauri::App,
    window: &tauri::WebviewWindow,
    saved: Option<&settings::WindowGeometry>,
) {
    let monitors = app.available_monitors().unwrap_or_default();
    let saved_on_monitor = saved.and_then(|geometry| {
        monitors.iter().find(|monitor| {
            let work = monitor.work_area();
            settings::point_in_rect(
                geometry.x,
                geometry.y,
                work.position.x,
                work.position.y,
                work.size.width,
                work.size.height,
            )
        })
    });
    let monitor = saved_on_monitor
        .cloned()
        .or_else(|| window.current_monitor().ok().flatten())
        .or_else(|| app.primary_monitor().ok().flatten());
    let Some(monitor) = monitor else { return; };

    let work = monitor.work_area();
    let (width, height) = settings::clamp_size(
        saved.map_or(900.0, |geometry| geometry.width),
        saved.map_or(810.0, |geometry| geometry.height),
        work.size.width,
        work.size.height,
        monitor.scale_factor(),
        720.0,
        520.0,
    );
    let _ = window.set_size(tauri::LogicalSize::new(width, height));

    match saved {
        Some(geometry) if saved_on_monitor.is_some() => {
            let scale = monitor.scale_factor();
            let physical_w = (width * scale).round() as i32;
            let physical_h = (height * scale).round() as i32;
            let max_x = work.position.x + work.size.width as i32 - physical_w;
            let max_y = work.position.y + work.size.height as i32 - physical_h;
            let x = geometry.x.clamp(work.position.x, max_x.max(work.position.x));
            let y = geometry.y.clamp(work.position.y, max_y.max(work.position.y));
            let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
        }
        _ => {
            let _ = window.center();
        }
    }
}
