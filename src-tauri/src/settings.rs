use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::paths;

/// App-level preferences — separate file from luma-lite's `config.json`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct StartupSettings {
    pub start_with_windows: bool,
    pub start_minimized: bool,
    pub close_to_tray: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppearanceSettings {
    pub theme: String,
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            theme: "midnight-blue".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WindowGeometry {
    /// Logical width/height (DPI-independent).
    pub width: f64,
    pub height: f64,
    /// Physical screen coordinates (exact across mixed-DPI monitors).
    pub x: i32,
    pub y: i32,
}

impl Default for WindowGeometry {
    fn default() -> Self {
        Self {
            width: 900.0,
            height: 810.0,
            x: 0,
            y: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct PanelSettings {
    pub startup: StartupSettings,
    pub appearance: AppearanceSettings,
    /// UI language: `"en"`, `"es"` or `""` (auto-detect on first run).
    pub language: String,
    /// Last window geometry; `None` on first run → default 900x810 centered.
    pub window: Option<WindowGeometry>,
}

/// Partial update received from the frontend (only the provided fields change).
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PartialSettings {
    pub startup: Option<PartialStartup>,
    pub appearance: Option<PartialAppearance>,
    pub language: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PartialStartup {
    pub start_with_windows: Option<bool>,
    pub start_minimized: Option<bool>,
    pub close_to_tray: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PartialAppearance {
    pub theme: Option<String>,
}

pub fn settings_path() -> std::path::PathBuf {
    paths::app_data_dir().join("panel-settings.json")
}

pub fn load() -> PanelSettings {
    let path = settings_path();
    if path.exists() {
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    } else {
        PanelSettings::default()
    }
}

fn save(settings: &PanelSettings) -> Result<(), String> {
    let json = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("Failed to serialize settings: {e}"))?;
    paths::atomic_write(&settings_path(), &json)
}

static PENDING_GEOMETRY: Mutex<Option<WindowGeometry>> = Mutex::new(None);

/// Snapshot the live geometry of `window` into the exit-time cache. Called for
/// every `Moved`/`Resized` event so the position survives window destruction.
pub fn capture_geometry(window: &tauri::Window<tauri::Wry>, event: &tauri::WindowEvent) {
    match event {
        tauri::WindowEvent::Moved(_) | tauri::WindowEvent::Resized(_) => {}
        _ => return,
    }
    if window.label() != "main" || window.is_minimized().unwrap_or(true) {
        return;
    }
    let Ok(position) = window.outer_position() else { return; };
    let Ok(size) = window.outer_size() else { return; };
    if size.width == 0 || size.height == 0 {
        return;
    }
    if position.x < -10_000 || position.y < -10_000 {
        return;
    }
    let scale = window.scale_factor().unwrap_or(1.0);
    let geometry = WindowGeometry {
        width: size.width as f64 / scale,
        height: size.height as f64 / scale,
        x: position.x,
        y: position.y,
    };
    if let Ok(mut pending) = PENDING_GEOMETRY.lock() {
        *pending = Some(geometry);
    }
}

/// Persist the cached geometry if it differs from the saved one. Fired on
/// `ExitRequested` and again on `Exit`; the second call is a no-op.
pub fn flush_geometry() {
    let geometry = match PENDING_GEOMETRY.lock() {
        Ok(mut pending) => pending.take(),
        Err(_) => return,
    };
    let Some(geometry) = geometry else { return; };
    let mut settings = load();
    if settings.window.as_ref() == Some(&geometry) {
        return;
    }
    settings.window = Some(geometry);
    let _ = save(&settings);
}

/// Clamp a desired logical window size into a physical work area, leaving a
/// 16px logical margin per side. The UI minimum wins when it still fits,
/// otherwise the work area wins (tiny screens).
pub fn clamp_size(
    desired_w: f64,
    desired_h: f64,
    work_w: u32,
    work_h: u32,
    scale: f64,
    min_w: f64,
    min_h: f64,
) -> (f64, f64) {
    let scale = if scale.is_finite() && scale > 0.0 { scale } else { 1.0 };
    let max_w = ((work_w as f64 / scale) - 32.0).max(1.0);
    let max_h = ((work_h as f64 / scale) - 32.0).max(1.0);
    let mut w = desired_w.min(max_w);
    let mut h = desired_h.min(max_h);
    if w < min_w && min_w <= max_w {
        w = min_w;
    }
    if h < min_h && min_h <= max_h {
        h = min_h;
    }
    (w.max(1.0), h.max(1.0))
}

pub fn point_in_rect(x: i32, y: i32, rx: i32, ry: i32, rw: u32, rh: u32) -> bool {
    x >= rx
        && y >= ry
        && (x as i64) < rx as i64 + rw as i64
        && (y as i64) < ry as i64 + rh as i64
}

/// Mirror `start_with_windows` into the HKCU Run key (Windows only).
#[cfg(target_os = "windows")]
fn sync_autostart(enabled: bool) -> Result<(), String> {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_SET_VALUE};
    use winreg::RegKey;

    const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const VALUE_NAME: &str = "LumaForgePanel";

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let run_key = hkcu
        .open_subkey_with_flags(RUN_KEY, KEY_SET_VALUE)
        .map_err(|e| format!("Failed to open Run registry key: {e}"))?;

    if enabled {
        let exe_path = std::env::current_exe()
            .map_err(|e| format!("Failed to get executable path: {e}"))?;
        run_key
            .set_value(VALUE_NAME, &exe_path.to_string_lossy().to_string())
            .map_err(|e| format!("Failed to set auto-start registry value: {e}"))?;
    } else {
        let _ = run_key.delete_value(VALUE_NAME);
    }

    Ok(())
}

/// Mirror the autostart setting into an XDG autostart entry (Linux only) —
/// the equivalent of the Windows Run key.
#[cfg(target_os = "linux")]
fn sync_autostart(enabled: bool) -> Result<(), String> {
    const ENTRY_NAME: &str = "lumaforge-panel.desktop";
    const MARKER: &str = "Name=LumaForge Panel";

    let entry_path = dirs::config_dir()
        .ok_or_else(|| "Config directory not found".to_string())?
        .join("autostart")
        .join(ENTRY_NAME);

    if enabled {
        let exe_path = std::env::current_exe()
            .map_err(|e| format!("Failed to get executable path: {e}"))?;
        let content = format!(
            "[Desktop Entry]\n\
             Type=Application\n\
             Name=LumaForge Panel\n\
             Comment=LumaForge control panel\n\
             Exec=\"{}\"\n\
             Terminal=false\n\
             X-GNOME-Autostart-enabled=true\n",
            exe_path.display()
        );
        if let Some(parent) = entry_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create autostart directory: {e}"))?;
        }
        std::fs::write(&entry_path, content)
            .map_err(|e| format!("Failed to write autostart entry: {e}"))?;
    } else if entry_path.exists() {
        // Only delete the entry if it is ours.
        let ours = std::fs::read_to_string(&entry_path)
            .map(|c| c.contains(MARKER))
            .unwrap_or(false);
        if ours {
            let _ = std::fs::remove_file(&entry_path);
        }
    }

    Ok(())
}

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
fn sync_autostart(_enabled: bool) -> Result<(), String> {
    Ok(())
}

pub fn get_settings() -> PanelSettings {
    load()
}

pub fn update_settings(partial: PartialSettings) -> Result<PanelSettings, String> {
    let mut settings = load();
    let mut autostart_changed: Option<bool> = None;

    if let Some(startup) = partial.startup {
        if let Some(value) = startup.start_with_windows {
            settings.startup.start_with_windows = value;
            autostart_changed = Some(value);
        }
        if let Some(value) = startup.start_minimized {
            settings.startup.start_minimized = value;
        }
        if let Some(value) = startup.close_to_tray {
            settings.startup.close_to_tray = value;
        }
    }

    if let Some(appearance) = partial.appearance {
        if let Some(theme) = appearance.theme {
            settings.appearance.theme = theme;
        }
    }

    if let Some(language) = partial.language {
        settings.language = language;
        crate::i18n::init_from_settings(&settings.language);
    }

    if let Some(enabled) = autostart_changed {
        sync_autostart(enabled)?;
    }

    save(&settings)?;
    Ok(settings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_keeps_default_on_large_screen() {
        let (w, h) = clamp_size(900.0, 810.0, 1920, 1032, 1.0, 720.0, 520.0);
        assert_eq!((w, h), (900.0, 810.0));
    }

    #[test]
    fn clamp_shrinks_height_to_work_area() {
        let (w, h) = clamp_size(900.0, 810.0, 1366, 720, 1.0, 720.0, 520.0);
        assert_eq!(w, 900.0);
        assert_eq!(h, 688.0);
    }

    #[test]
    fn clamp_applies_physical_margin_on_hidpi() {
        let (w, h) = clamp_size(900.0, 810.0, 2560, 1440, 2.0, 720.0, 520.0);
        assert_eq!(w, 900.0);
        assert_eq!(h, 688.0);
    }

    #[test]
    fn clamp_upsizes_to_ui_minimum() {
        let (w, h) = clamp_size(300.0, 200.0, 1920, 1032, 1.0, 720.0, 520.0);
        assert_eq!((w, h), (720.0, 520.0));
    }

    #[test]
    fn clamp_accepts_sub_min_when_work_area_is_smaller() {
        let (w, h) = clamp_size(900.0, 810.0, 700, 500, 1.0, 720.0, 520.0);
        assert_eq!(w, 668.0);
        assert_eq!(h, 468.0);
    }

    #[test]
    fn point_in_rect_detects_hits_and_misses() {
        assert!(point_in_rect(100, 100, 0, 0, 1920, 1040));
        assert!(point_in_rect(5000, 5000, 4000, 4000, 1920, 1040));
        assert!(!point_in_rect(-32000, -32000, 0, 0, 1920, 1040));
        assert!(!point_in_rect(1920, 0, 0, 0, 1920, 1040));
        assert!(!point_in_rect(0, 1040, 0, 0, 1920, 1040));
    }
}
