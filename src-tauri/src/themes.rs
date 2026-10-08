use serde::Serialize;
use serde_json::{Map, Value};
use std::fs;
use std::path::{Path, PathBuf};

use crate::panel::PanelResult;
use crate::paths;

// ---------------------------------------------------------------------------
// Types surfaced to the frontend
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeSlider {
    pub css_variable: String,
    pub min: f64,
    pub max: f64,
    pub step: f64,
    pub unit: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeCondition {
    pub key: String,
    pub description: String,
    pub tab: String,
    pub section: String,
    pub default: Value,
    /// Dropdown option names; `None` for slider conditions.
    pub values: Option<Vec<String>>,
    pub slider: Option<ThemeSlider>,
    /// Persisted selection from active.json; `None` = theme default.
    pub selected: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeInfo {
    pub id: String,
    pub name: String,
    pub author: String,
    pub description: String,
    pub version: String,
    pub tags: Vec<String>,
    pub preview_path: Option<String>,
    pub conditions: Vec<ThemeCondition>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemesState {
    /// Directory name of the active theme; empty string = theming disabled.
    pub active: String,
    pub themes: Vec<ThemeInfo>,
}

// ---------------------------------------------------------------------------
// Paths + active.json (Millennium format, shared with the CDP proxy)
// ---------------------------------------------------------------------------

fn themes_dir() -> PathBuf {
    paths::app_data_dir().join("themes")
}

fn read_active_doc() -> Value {
    let path = themes_dir().join("active.json");
    let raw = fs::read_to_string(&path).unwrap_or_default();
    let content = raw.trim_start_matches('\u{FEFF}');
    serde_json::from_str(content).unwrap_or_else(|_| Value::Object(Map::new()))
}

fn write_active_doc(doc: &Value) -> Result<(), String> {
    let path = themes_dir().join("active.json");
    let json =
        serde_json::to_string_pretty(doc).map_err(|e| format!("serialize active.json: {e}"))?;
    paths::atomic_write(&path, &json)
}

fn active_name(doc: &Value) -> String {
    if let Some(name) = doc
        .get("themes")
        .and_then(|t| t.get("activeTheme"))
        .and_then(|v| v.as_str())
    {
        return name.to_string();
    }
    if let Some(name) = doc.get("theme").and_then(|v| v.as_str()) {
        return name.to_string();
    }
    String::new()
}

/// Set `themes.activeTheme`, preserving conditions and every other key.
/// Drops the legacy `{"theme": ...}` key so both formats never disagree.
fn set_active_name(name: &str) -> Result<(), String> {
    let doc = read_active_doc();
    let mut root = match doc {
        Value::Object(map) => map,
        _ => Map::new(),
    };

    let themes_val = root
        .entry("themes")
        .or_insert_with(|| Value::Object(Map::new()));
    if let Some(themes) = themes_val.as_object_mut() {
        themes.insert("activeTheme".into(), Value::String(name.to_string()));
        themes
            .entry("conditions")
            .or_insert_with(|| Value::Object(Map::new()));
    }
    root.remove("theme");

    write_active_doc(&Value::Object(root))
}

fn read_selections(doc: &Value) -> std::collections::HashMap<String, std::collections::HashMap<String, String>> {
    let mut out = std::collections::HashMap::new();
    let Some(conditions) = doc
        .get("themes")
        .and_then(|t| t.get("conditions"))
        .and_then(|c| c.as_object())
    else {
        return out;
    };
    for (theme, conds) in conditions {
        let Some(obj) = conds.as_object() else { continue };
        let map = out.entry(theme.clone()).or_insert_with(Default::default);
        for (key, value) in obj {
            if let Some(v) = value.as_str() {
                map.insert(key.clone(), v.to_string());
            }
        }
    }
    out
}

/// Mutate `themes.conditions` in active.json. `value: None` removes the key
/// (reset to the theme default).
fn write_condition(theme_id: &str, key: &str, value: Option<&str>) -> Result<(), String> {
    let doc = read_active_doc();
    let mut root = match doc {
        Value::Object(map) => map,
        _ => Map::new(),
    };

    let themes_val = root
        .entry("themes")
        .or_insert_with(|| Value::Object(Map::new()));
    let themes = themes_val
        .as_object_mut()
        .ok_or("active.json: themes is not an object")?;
    let conditions_val = themes
        .entry("conditions")
        .or_insert_with(|| Value::Object(Map::new()));
    let conditions = conditions_val
        .as_object_mut()
        .ok_or("active.json: conditions is not an object")?;
    let theme_conds_val = conditions
        .entry(theme_id.to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    let theme_conds = theme_conds_val
        .as_object_mut()
        .ok_or("active.json: theme conditions not an object")?;

    match value {
        Some(v) => {
            theme_conds.insert(key.to_string(), Value::String(v.to_string()));
        }
        None => {
            theme_conds.remove(key);
        }
    }

    write_active_doc(&Value::Object(root))
}

// ---------------------------------------------------------------------------
// skin.json inspection
// ---------------------------------------------------------------------------

fn skin_str(skin: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|k| skin.get(*k).and_then(|v| v.as_str()))
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
}

fn resolve_preview(dir: &Path, skin: &Value) -> Option<PathBuf> {
    for key in [
        "header_image",
        "headerImage",
        "splash_image",
        "preview",
        "PreviewImage",
    ] {
        if let Some(value) = skin.get(key).and_then(|v| v.as_str()) {
            if value.is_empty() || value.starts_with("http") {
                continue;
            }
            let clean = value.strip_prefix("./").unwrap_or(value);
            let path = dir.join(clean);
            if path.is_file() {
                return Some(path);
            }
        }
    }

    let mut candidates: Vec<PathBuf> = fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && matches!(
                    p.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()),
                    Some(ref ext) if matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "webp" | "gif")
                )
        })
        .collect();

    candidates.sort_by_key(|p| {
        let name = p
            .file_name()
            .map(|n| n.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let rank = if name.contains("header") || name.contains("banner") || name.contains("preview")
        {
            0
        } else if name.contains("splash") || name.contains("main") || name.contains("cover") {
            1
        } else {
            2
        };
        (rank, name)
    });

    candidates.into_iter().next()
}

fn parse_conditions(
    skin: &Value,
    selections: Option<&std::collections::HashMap<String, String>>,
) -> Vec<ThemeCondition> {
    let Some(conditions) = skin
        .get("Conditions")
        .or_else(|| skin.get("conditions"))
        .and_then(|c| c.as_object())
    else {
        return Vec::new();
    };

    conditions
        .iter()
        .map(|(key, cond)| {
            let values = cond
                .get("values")
                .or_else(|| cond.get("Values"))
                .and_then(|v| v.as_object())
                .map(|obj| obj.keys().cloned().collect());

            let slider = cond.get("slider").or_else(|| cond.get("Slider")).and_then(|s| {
                Some(ThemeSlider {
                    css_variable: s
                        .get("cssVariable")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string(),
                    min: s.get("min").and_then(|v| v.as_f64()).unwrap_or(0.0),
                    max: s.get("max").and_then(|v| v.as_f64()).unwrap_or(100.0),
                    step: s.get("step").and_then(|v| v.as_f64()).unwrap_or(0.0),
                    unit: s
                        .get("unit")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                })
            });

            ThemeCondition {
                key: key.clone(),
                description: skin_str(cond, &["description", "Description"]).unwrap_or_default(),
                tab: skin_str(cond, &["tab", "Tab"]).unwrap_or_default(),
                section: skin_str(cond, &["section", "Section"]).unwrap_or_default(),
                default: cond.get("default").cloned().unwrap_or(Value::Null),
                values,
                slider,
                selected: selections.and_then(|s| s.get(key).cloned()),
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Public operations
// ---------------------------------------------------------------------------

pub fn get_state() -> ThemesState {
    let doc = read_active_doc();
    let active = active_name(&doc);
    let selections = read_selections(&doc);
    let dir = themes_dir();

    let mut themes: Vec<ThemeInfo> = Vec::new();
    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let Ok(raw) = fs::read_to_string(path.join("skin.json")) else {
                continue;
            };
            let skin: Value = match serde_json::from_str(raw.trim_start_matches('\u{FEFF}')) {
                Ok(v) => v,
                Err(_) => continue,
            };
            let Some(id) = path.file_name().map(|n| n.to_string_lossy().to_string()) else {
                continue;
            };

            let info = ThemeInfo {
                name: skin_str(&skin, &["name", "Name"]).unwrap_or_else(|| id.clone()),
                author: skin_str(&skin, &["author", "Author"]).unwrap_or_default(),
                description: skin_str(&skin, &["description", "Description"]).unwrap_or_default(),
                version: skin_str(&skin, &["version", "Version"]).unwrap_or_default(),
                tags: skin
                    .get("tags")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|t| t.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default(),
                preview_path: resolve_preview(&path, &skin)
                    .map(|p| p.to_string_lossy().to_string()),
                conditions: parse_conditions(&skin, selections.get(&id)),
                id,
            };
            themes.push(info);
        }
    }

    themes.sort_by(|a, b| a.id.to_lowercase().cmp(&b.id.to_lowercase()));
    ThemesState { active, themes }
}

fn validate_theme(id: &str) -> Result<(), String> {
    if id.is_empty() || id.contains('/') || id.contains('\\') || id.contains("..") {
        return Err(crate::i18n::t("theme.invalid_name", &[]));
    }
    if !themes_dir().join(id).join("skin.json").is_file() {
        return Err(crate::i18n::t("theme.not_installed", &[id]));
    }
    Ok(())
}

/// Best-effort notify: ask the running proxy to re-export the theme manifest
/// and signal cef_hook. When Steam/the proxy is not running this fails — the
/// proxy exports on its next start, so the disk write alone is enough.
/// A proxy older than the `sync-theme` command answers "Unknown command",
/// which counts as not synced (the panel then hints at the next launch).
fn sync_proxy() -> std::result::Result<(), ()> {
    let response = ipc_sync().map_err(|_| ())?;
    if response.contains(r#""status":"ok""#) {
        Ok(())
    } else {
        Err(())
    }
}

fn sync_suffix() -> String { crate::i18n::t("theme.sync_suffix", &[]) }

pub fn activate(id: &str) -> Result<PanelResult, String> {
    validate_theme(id)?;
    set_active_name(id)?;
    let suffix = if sync_proxy().is_ok() { String::new() } else { sync_suffix() };
    Ok(PanelResult {
        ok: true,
        message: crate::i18n::t("theme.activated", &[id, &suffix]),
        restart_required: false,
    })
}

pub fn deactivate() -> Result<PanelResult, String> {
    set_active_name("")?;
    let suffix = if sync_proxy().is_ok() { String::new() } else { sync_suffix() };
    Ok(PanelResult {
        ok: true,
        message: crate::i18n::t("theme.disabled", &[&suffix]),
        restart_required: false,
    })
}

pub fn set_condition(theme_id: &str, key: &str, value: &str) -> Result<PanelResult, String> {
    validate_theme(theme_id)?;
    write_condition(theme_id, key, Some(value))?;
    let suffix = if sync_proxy().is_ok() { String::new() } else { sync_suffix() };
    Ok(PanelResult {
        ok: true,
        message: crate::i18n::t("theme.condition_updated", &[&suffix]),
        restart_required: false,
    })
}

pub fn reset_condition(theme_id: &str, key: &str) -> Result<PanelResult, String> {
    validate_theme(theme_id)?;
    write_condition(theme_id, key, None)?;
    let suffix = if sync_proxy().is_ok() { String::new() } else { sync_suffix() };
    Ok(PanelResult {
        ok: true,
        message: crate::i18n::t("theme.condition_reset", &[&suffix]),
        restart_required: false,
    })
}

// ---------------------------------------------------------------------------
// IPC client → CDP proxy (`sync-theme`)
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
fn ipc_sync() -> Result<String, String> {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{CreateFileA, ReadFile, WriteFile};

    const NAME: &[u8] = b"\\\\.\\pipe\\lumalite_core\0";
    const GENERIC_READ_: u32 = 0x8000_0000;
    const GENERIC_WRITE_: u32 = 0x4000_0000;
    const OPEN_EXISTING: u32 = 3;

    let mut handle = INVALID_HANDLE_VALUE;
    for attempt in 0..4 {
        handle = unsafe {
            CreateFileA(
                NAME.as_ptr(),
                GENERIC_READ_ | GENERIC_WRITE_,
                0,
                std::ptr::null(),
                OPEN_EXISTING,
                0,
                std::ptr::null_mut(),
            )
        };
        if handle != INVALID_HANDLE_VALUE {
            break;
        }
        if attempt < 3 {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }
    if handle == INVALID_HANDLE_VALUE {
        return Err(crate::i18n::t("err.cdp_not_running", &[]));
    }

    let cmd = b"sync-theme";
    let mut written = 0u32;
    let ok = unsafe {
        WriteFile(
            handle,
            cmd.as_ptr(),
            cmd.len() as u32,
            &mut written,
            std::ptr::null_mut(),
        )
    };

    let mut buffer = [0u8; 1024];
    let mut read = 0u32;
    if ok != 0 {
        unsafe {
            ReadFile(
                handle,
                buffer.as_mut_ptr(),
                buffer.len() as u32,
                &mut read,
                std::ptr::null_mut(),
            )
        };
    }
    unsafe { CloseHandle(handle) };

    if ok == 0 || read == 0 {
        return Err(crate::i18n::t("err.cdp_no_response", &[]));
    }
    Ok(String::from_utf8_lossy(&buffer[..read as usize]).into_owned())
}

#[cfg(target_os = "linux")]
fn ipc_sync() -> Result<String, String> {
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;
    use std::time::Duration;

    let mut stream = UnixStream::connect("/tmp/lumalite_core.sock")
        .map_err(|e| format!("{}: {e}", crate::i18n::t("err.cdp_not_running", &[])))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(1)))
        .map_err(|e| e.to_string())?;
    stream
        .write_all(b"sync-theme")
        .map_err(|e| e.to_string())?;
    let mut buffer = [0u8; 1024];
    let read = stream.read(&mut buffer).map_err(|e| e.to_string())?;
    if read == 0 {
        return Err(crate::i18n::t("err.cdp_no_response", &[]));
    }
    Ok(String::from_utf8_lossy(&buffer[..read]).into_owned())
}

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
fn ipc_sync() -> Result<String, String> {
    Err(crate::i18n::t("err.ipc_unsupported", &[]))
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn themes_get() -> ThemesState {
    get_state()
}

#[tauri::command]
pub fn theme_activate(id: String) -> Result<PanelResult, String> {
    activate(&id)
}

#[tauri::command]
pub fn theme_deactivate() -> Result<PanelResult, String> {
    deactivate()
}

#[tauri::command]
pub fn theme_set_condition(
    theme_id: String,
    key: String,
    value: String,
) -> Result<PanelResult, String> {
    set_condition(&theme_id, &key, &value)
}

#[tauri::command]
pub fn theme_reset_condition(theme_id: String, key: String) -> Result<PanelResult, String> {
    reset_condition(&theme_id, &key)
}
