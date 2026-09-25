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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct PanelSettings {
    pub startup: StartupSettings,
    pub appearance: AppearanceSettings,
}

/// Partial update received from the frontend (only the provided fields change).
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PartialSettings {
    pub startup: Option<PartialStartup>,
    pub appearance: Option<PartialAppearance>,
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

#[cfg(not(target_os = "windows"))]
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

    if let Some(enabled) = autostart_changed {
        sync_autostart(enabled)?;
    }

    save(&settings)?;
    Ok(settings)
}
