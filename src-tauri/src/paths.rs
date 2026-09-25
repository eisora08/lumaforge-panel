use std::path::PathBuf;

/// Canonical application data directory: `{local_data_dir}/LumaForge`.
pub fn app_data_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("LumaForge")
}

/// `{local_data_dir}/LumaForge/thirdparty` — tool payloads + state file.
pub fn thirdparty_dir() -> PathBuf {
    app_data_dir().join("thirdparty")
}

/// `{local_data_dir}/LumaForge/thirdparty/thirdparty-state.json`
/// (same file the CDP proxy reads and writes).
pub fn thirdparty_state_path() -> PathBuf {
    thirdparty_dir().join("thirdparty-state.json")
}

/// `{local_data_dir}/LumaForge/plugins` — runtime plugin directories.
pub fn plugins_dir() -> PathBuf {
    app_data_dir().join("plugins")
}

/// Temp directory used for downloads and extractions.
pub fn temp_dir() -> PathBuf {
    std::env::temp_dir()
}

/// Atomically write `content` to `path` via a temporary file + rename.
pub fn atomic_write(path: &std::path::Path, content: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory {}: {e}", parent.display()))?;
    }
    let temp_path = path.with_extension("json.tmp");
    std::fs::write(&temp_path, content)
        .map_err(|e| format!("Failed to write temp file {}: {e}", temp_path.display()))?;
    std::fs::rename(&temp_path, path)
        .map_err(|e| format!("Failed to rename {} -> {}: {e}", temp_path.display(), path.display()))?;
    Ok(())
}

/// Try to detect the Steam installation root.
///
/// Priority: `LUMAFORGE_STEAM_ROOT` / `STEAM_PATH` env vars, Windows registry,
/// then well-known install paths.
pub fn detect_steam_root() -> Option<PathBuf> {
    for key in ["LUMAFORGE_STEAM_ROOT", "STEAM_PATH"] {
        if let Ok(value) = std::env::var(key) {
            let path = PathBuf::from(&value);
            if path.exists() {
                return Some(path);
            }
        }
    }

    #[cfg(target_os = "windows")]
    if let Some(path) = detect_steam_from_registry() {
        return Some(path);
    }

    let candidates: Vec<PathBuf> = vec![
        PathBuf::from(r"C:\Program Files (x86)\Steam"),
        PathBuf::from(r"C:\Program Files\Steam"),
        dirs::home_dir()
            .map(|h| h.join(".steam/steam"))
            .unwrap_or_default(),
        dirs::home_dir()
            .map(|h| h.join(".local/share/Steam"))
            .unwrap_or_default(),
        dirs::home_dir()
            .map(|h| h.join(".var/app/com.valvesoftware.Steam/data/Steam"))
            .unwrap_or_default(),
    ];

    candidates.into_iter().find(|c| c.exists())
}

#[cfg(target_os = "windows")]
fn detect_steam_from_registry() -> Option<PathBuf> {
    use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ};
    use winreg::RegKey;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let subkeys = [
        r"SOFTWARE\WOW6432Node\Valve\Steam",
        r"SOFTWARE\Valve\Steam",
    ];

    for subkey in subkeys {
        if let Ok(key) = hklm.open_subkey_with_flags(subkey, KEY_READ) {
            if let Ok(value) = key.get_value::<String, _>("InstallPath") {
                let path = PathBuf::from(value);
                if path.exists() {
                    return Some(path);
                }
            }
        }
    }

    None
}
