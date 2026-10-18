use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::github;
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

fn validate_id(id: &str) -> Result<(), String> {
    if id.is_empty() || id.contains('/') || id.contains('\\') || id.contains("..") {
        return Err(crate::i18n::t("theme.invalid_name", &[]));
    }
    Ok(())
}

fn validate_theme(id: &str) -> Result<(), String> {
    validate_id(id)?;
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
// Catalog — SteamBrew API (steambrew.app/api/v2) + on-disk cache
// ---------------------------------------------------------------------------

const CATALOG_API: &str = "https://steambrew.app/api/v2";
const CATALOG_TTL_SECS: u64 = 30 * 60;
const CATALOG_UA: &str = concat!(
    env!("CARGO_PKG_NAME"),
    "/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/eisora08/lumaforge-panel)"
);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogTheme {
    pub id: String,
    pub name: String,
    pub owner: String,
    pub repo: String,
    pub description: String,
    pub version: String,
    pub tags: Vec<String>,
    pub download: String,
    pub preview: Option<String>,
    #[serde(default)]
    pub preview_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogResult {
    pub entries: Vec<CatalogTheme>,
    /// "live" = fetched from the API, "cache" = disk fallback, "empty" = unavailable.
    pub source: &'static str,
}

struct CatalogMemory {
    fetched_at: Instant,
    entries: Vec<CatalogTheme>,
}

fn catalog_memory() -> &'static Mutex<Option<CatalogMemory>> {
    static CELL: OnceLock<Mutex<Option<CatalogMemory>>> = OnceLock::new();
    CELL.get_or_init(|| Mutex::new(None))
}

fn catalog_cache_path() -> PathBuf {
    paths::app_data_dir().join("theme-catalog.json")
}

fn previews_dir() -> PathBuf {
    paths::app_data_dir().join("theme-previews")
}

/// Directory-safe id: lowercased `[a-z0-9-]`, dashes collapsed, dashes trimmed.
fn slug(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let trimmed = out.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "theme".to_string()
    } else {
        trimmed
    }
}

fn valid_repo_ref(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

fn row_github(row: &Value) -> Option<(&str, &str)> {
    let owner = row["data"]["github"]["owner"].as_str()?;
    let repo = row["data"]["github"]["repo"].as_str()?;
    Some((owner, repo))
}

fn parse_catalog(raw: &str) -> Result<Vec<CatalogTheme>, String> {
    let rows: Vec<Value> = serde_json::from_str(raw).map_err(|e| format!("catalog parse: {e}"))?;

    // First pass: a repo slug shared by several owners needs an owner prefix.
    let mut slug_counts: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
    for row in &rows {
        if let Some((owner, repo)) = row_github(row) {
            if valid_repo_ref(owner) && valid_repo_ref(repo) {
                *slug_counts.entry(slug(repo)).or_insert(0) += 1;
            }
        }
    }

    let mut entries: Vec<CatalogTheme> = Vec::new();
    for row in &rows {
        let Some((owner, repo)) = row_github(row) else {
            continue;
        };
        if !valid_repo_ref(owner) || !valid_repo_ref(repo) {
            continue;
        }

        let download = row["download"].as_str().unwrap_or_default().to_string();
        let archive_prefix = format!("https://github.com/{owner}/{repo}/archive/");
        if !download.starts_with(&archive_prefix) {
            continue;
        }

        let preview = ["splash_image", "header_image"]
            .iter()
            .filter_map(|key| row.get(*key).and_then(|value| value.as_str()))
            .find(|url| url.starts_with("https://"))
            .map(|url| url.to_string());

        let base = slug(repo);
        let id = if slug_counts.get(&base).copied().unwrap_or(0) > 1 {
            slug(&format!("{owner}-{repo}"))
        } else {
            base
        };

        let skin = &row["skin_data"];
        let name = row["name"]
            .as_str()
            .filter(|value| !value.is_empty())
            .or_else(|| skin["name"].as_str())
            .filter(|value| !value.is_empty())
            .unwrap_or(repo)
            .to_string();

        entries.push(CatalogTheme {
            id,
            name,
            owner: owner.to_string(),
            repo: repo.to_string(),
            description: row["description"].as_str().unwrap_or_default().to_string(),
            version: row["version"].as_str().unwrap_or_default().to_string(),
            tags: row["tags"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|tag| tag.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default(),
            download,
            preview,
            preview_path: None,
        });
    }

    // Final uniqueness pass (owner-prefixed slugs can still collide).
    let mut seen = std::collections::HashSet::new();
    for entry in &mut entries {
        let mut candidate = entry.id.clone();
        let mut n = 2;
        while !seen.insert(candidate.clone()) {
            candidate = format!("{}-{n}", entry.id);
            n += 1;
        }
        entry.id = candidate;
    }
    Ok(entries)
}

fn fnv1a_hex(value: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

fn preview_filename(url: &str) -> Option<String> {
    let parsed = reqwest::Url::parse(url).ok()?;
    if parsed.scheme() != "https" {
        return None;
    }
    let ext = parsed.path().rsplit('.').next()?.to_ascii_lowercase();
    if !matches!(ext.as_str(), "webp" | "png" | "jpg" | "jpeg" | "gif") {
        return None;
    }
    Some(format!("{}.{ext}", fnv1a_hex(url)))
}

fn build_result(mut entries: Vec<CatalogTheme>, source: &'static str) -> CatalogResult {
    for entry in &mut entries {
        entry.preview_path = entry
            .preview
            .as_deref()
            .and_then(preview_filename)
            .and_then(|name| {
                let path = previews_dir().join(&name);
                path.is_file()
                    .then(|| path.to_string_lossy().to_string())
            });
    }
    CatalogResult { entries, source }
}

/// Drop cached previews that no catalog entry references anymore.
fn prune_previews(entries: &[CatalogTheme]) {
    let keep: std::collections::HashSet<String> = entries
        .iter()
        .filter_map(|entry| entry.preview.as_deref())
        .filter_map(preview_filename)
        .collect();
    if let Ok(dir_entries) = fs::read_dir(previews_dir()) {
        for dir_entry in dir_entries.flatten() {
            let name = dir_entry.file_name().to_string_lossy().to_string();
            if name.ends_with(".part") || keep.contains(&name) {
                continue;
            }
            let _ = fs::remove_file(dir_entry.path());
        }
    }
}

fn catalog_client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .user_agent(CATALOG_UA)
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())
}

fn fetch_catalog_live() -> Result<Vec<CatalogTheme>, String> {
    let client = catalog_client()?;
    let response = client
        .get(CATALOG_API)
        .send()
        .map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!("HTTP {}", response.status()));
    }
    let body = response.text().map_err(|e| e.to_string())?;
    parse_catalog(&body)
}

/// Memory (TTL) -> live API (+ disk snapshot) -> disk -> empty.
fn load_catalog(force: bool) -> CatalogResult {
    if !force {
        if let Ok(cell) = catalog_memory().lock() {
            if let Some(mem) = cell.as_ref() {
                if mem.fetched_at.elapsed().as_secs() < CATALOG_TTL_SECS {
                    return build_result(mem.entries.clone(), "live");
                }
            }
        }
    }

    match fetch_catalog_live() {
        Ok(entries) => {
            if let Ok(mut cell) = catalog_memory().lock() {
                *cell = Some(CatalogMemory {
                    fetched_at: Instant::now(),
                    entries: entries.clone(),
                });
            }
            if let Ok(json) = serde_json::to_string(&entries) {
                let _ = paths::atomic_write(&catalog_cache_path(), &json);
            }
            prune_previews(&entries);
            build_result(entries, "live")
        }
        Err(_) => {
            // API unreachable: serve the last good copy from disk. Memory stays
            // empty so the next call retries the API instead of going stale.
            match fs::read_to_string(catalog_cache_path())
                .ok()
                .and_then(|raw| serde_json::from_str::<Vec<CatalogTheme>>(&raw).ok())
            {
                Some(entries) => build_result(entries, "cache"),
                None => CatalogResult {
                    entries: Vec::new(),
                    source: "empty",
                },
            }
        }
    }
}

pub fn fetch_preview(url: &str) -> Result<String, String> {
    let parsed = reqwest::Url::parse(url)
        .map_err(|e| crate::i18n::t("err.theme_preview_url", &[&e.to_string()]))?;
    let host = parsed.host_str().unwrap_or_default().to_string();
    if parsed.scheme() != "https"
        || !matches!(
            host.as_str(),
            "raw.githubusercontent.com" | "i.imgur.com"
        )
    {
        return Err(crate::i18n::t("err.theme_preview_host", &[&host]));
    }
    let filename =
        preview_filename(url).ok_or_else(|| crate::i18n::t("err.theme_preview_type", &[url]))?;
    let path = previews_dir().join(&filename);
    if path.is_file() {
        return Ok(path.to_string_lossy().to_string());
    }

    let client = reqwest::blocking::Client::builder()
        .user_agent(CATALOG_UA)
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|e| crate::i18n::t("err.theme_preview_fetch", &[&e.to_string()]))?;
    let response = client
        .get(url)
        .send()
        .map_err(|e| crate::i18n::t("err.theme_preview_fetch", &[&e.to_string()]))?;
    if !response.status().is_success() {
        let message = format!("HTTP {}", response.status());
        return Err(crate::i18n::t("err.theme_preview_fetch", &[&message]));
    }
    let bytes = response
        .bytes()
        .map_err(|e| crate::i18n::t("err.theme_preview_fetch", &[&e.to_string()]))?;
    if bytes.is_empty() {
        let message = "empty body".to_string();
        return Err(crate::i18n::t("err.theme_preview_fetch", &[&message]));
    }

    let _ = fs::create_dir_all(previews_dir());
    let part = previews_dir().join(format!("{filename}.part"));
    fs::write(&part, &bytes)
        .map_err(|e| crate::i18n::t("err.theme_preview_fetch", &[&e.to_string()]))?;
    if let Err(e) = fs::rename(&part, &path) {
        let _ = fs::remove_file(&part);
        if !path.is_file() {
            return Err(crate::i18n::t("err.theme_preview_fetch", &[&e.to_string()]));
        }
    }
    Ok(path.to_string_lossy().to_string())
}

fn find_theme_root(extract_dir: &Path) -> Option<PathBuf> {
    if extract_dir.join("skin.json").is_file() {
        return Some(extract_dir.to_path_buf());
    }
    let root =
        github::flatten_extracted_dir(extract_dir).unwrap_or_else(|| extract_dir.to_path_buf());
    if root.join("skin.json").is_file() {
        return Some(root);
    }
    find_skin_dir(&root, 0)
}

fn find_skin_dir(dir: &Path, depth: u8) -> Option<PathBuf> {
    if depth > 4 {
        return None;
    }
    let mut children: Vec<PathBuf> = fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    children.sort();
    for child in children {
        if child.join("skin.json").is_file() {
            return Some(child);
        }
        if let Some(found) = find_skin_dir(&child, depth + 1) {
            return Some(found);
        }
    }
    None
}

fn post_download_count(owner: &str, repo: &str) -> Result<(), String> {
    let client = reqwest::blocking::Client::builder()
        .user_agent(CATALOG_UA)
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;
    let payload = serde_json::json!({ "owner": owner, "repo": repo });
    let response = client
        .post(format!("{CATALOG_API}/download"))
        .json(&payload)
        .send()
        .map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(format!("HTTP {}", response.status()));
    }
    Ok(())
}

pub fn install(id: &str) -> Result<PanelResult, String> {
    validate_id(id)?;
    let catalog = load_catalog(false);
    let entry = catalog
        .entries
        .iter()
        .find(|entry| entry.id == id)
        .ok_or_else(|| crate::i18n::t("err.theme_unknown", &[id]))?
        .clone();

    if themes_dir().join(id).join("skin.json").is_file() {
        return Ok(PanelResult {
            ok: true,
            message: crate::i18n::t("theme.already_installed", &[&entry.name]),
            restart_required: false,
        });
    }

    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let temp_dir = paths::temp_dir().join(format!("panel_theme_{}_{stamp}", entry.id));
    fs::create_dir_all(&temp_dir).map_err(|e| e.to_string())?;

    let outcome = (|| -> Result<PanelResult, String> {
        let archive = temp_dir.join("theme.zip");
        github::download_file(&entry.download, &archive)
            .map_err(|e| crate::i18n::t("err.theme_download", &[&entry.name, &e.to_string()]))?;

        let extract_dir = temp_dir.join("extracted");
        github::extract_archive(&archive, "zip", &extract_dir)
            .map_err(|e| crate::i18n::t("err.theme_extract", &[&e.to_string()]))?;

        let source = find_theme_root(&extract_dir)
            .ok_or_else(|| crate::i18n::t("err.theme_no_skin", &[]))?;

        let target = themes_dir().join(id);
        github::copy_dir_recursive(&source, &target)?;

        // Best effort: credit the download on SteamBrew.
        let _ = post_download_count(&entry.owner, &entry.repo);

        Ok(PanelResult {
            ok: true,
            message: crate::i18n::t("theme.installed", &[&entry.name]),
            restart_required: false,
        })
    })();

    let _ = fs::remove_dir_all(&temp_dir);
    outcome
}

/// Drop `themes.conditions.{id}` from active.json, preserving everything else.
fn remove_theme_conditions(id: &str) -> Result<bool, String> {
    let doc = read_active_doc();
    let mut root = match doc {
        Value::Object(map) => map,
        _ => Map::new(),
    };
    let removed = root
        .get_mut("themes")
        .and_then(|themes| themes.get_mut("conditions"))
        .and_then(|conditions| conditions.as_object_mut())
        .map(|conditions| conditions.remove(id).is_some())
        .unwrap_or(false);
    if removed {
        write_active_doc(&Value::Object(root))?;
    }
    Ok(removed)
}

pub fn uninstall(id: &str) -> Result<PanelResult, String> {
    validate_id(id)?;
    if !themes_dir().join(id).join("skin.json").is_file() {
        return Err(crate::i18n::t("theme.not_installed", &[id]));
    }

    let was_active = active_name(&read_active_doc()) == id;
    if was_active {
        set_active_name("")?;
    }
    let conditions_removed = remove_theme_conditions(id)?;

    github::remove_dir_recursive(&themes_dir().join(id))?;

    let needs_sync = was_active || conditions_removed;
    let suffix = if needs_sync && sync_proxy().is_err() {
        sync_suffix()
    } else {
        String::new()
    };
    Ok(PanelResult {
        ok: true,
        message: crate::i18n::t("theme.uninstalled", &[id, &suffix]),
        restart_required: false,
    })
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

#[tauri::command]
pub fn themes_catalog(force: Option<bool>) -> CatalogResult {
    load_catalog(force.unwrap_or(false))
}

#[tauri::command]
pub fn theme_fetch_preview(url: String) -> Result<String, String> {
    fetch_preview(&url)
}

#[tauri::command]
pub fn theme_install(id: String) -> Result<PanelResult, String> {
    install(&id)
}

#[tauri::command]
pub fn theme_uninstall(id: String) -> Result<PanelResult, String> {
    uninstall(&id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_lowercases_and_collapses_dashes() {
        assert_eq!(slug("My_Theme.Name"), "my-theme-name");
        assert_eq!(slug("--x--"), "x");
        assert_eq!(slug("??"), "theme");
    }

    #[test]
    fn parse_catalog_filters_rows_and_derives_ids() {
        let raw = r#"[
            {
                "name": "",
                "version": "1.2.0",
                "description": "desc",
                "tags": ["dark"],
                "splash_image": "https://raw.githubusercontent.com/o/r/main/preview.webp",
                "download": "https://github.com/o/r/archive/refs/heads/main.zip",
                "data": { "github": { "owner": "o", "repo": "r" } },
                "skin_data": { "name": "Fallback Name" }
            },
            {
                "name": "Bad",
                "download": "https://evil.example/x.zip",
                "data": { "github": { "owner": "o", "repo": "bad" } }
            },
            {
                "name": "No repo",
                "download": "https://github.com/o/x/archive/refs/heads/main.zip"
            }
        ]"#;
        let entries = parse_catalog(raw).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "r");
        assert_eq!(entries[0].name, "Fallback Name");
        assert_eq!(entries[0].owner, "o");
        assert_eq!(entries[0].version, "1.2.0");
        assert_eq!(entries[0].tags, vec!["dark".to_string()]);
        assert_eq!(
            entries[0].preview.as_deref(),
            Some("https://raw.githubusercontent.com/o/r/main/preview.webp")
        );
    }

    #[test]
    fn parse_catalog_prefixes_duplicate_repo_slugs() {
        let row = |owner: &str, repo: &str| {
            format!(
                r#"{{
                    "download": "https://github.com/{owner}/{repo}/archive/refs/heads/main.zip",
                    "data": {{ "github": {{ "owner": "{owner}", "repo": "{repo}" }} }}
                }}"#
            )
        };
        let raw = format!(
            "[{},{},{}]",
            row("alpha", "theme"),
            row("beta", "theme"),
            row("alpha", "other")
        );
        let entries = parse_catalog(&raw).unwrap();
        let ids: Vec<&str> = entries.iter().map(|entry| entry.id.as_str()).collect();
        assert_eq!(ids, vec!["alpha-theme", "beta-theme", "other"]);
    }
}
