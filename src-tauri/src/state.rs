use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::paths;

/// One entry of `thirdparty-state.json` — mirrors the CDP proxy's format so
/// both apps can read/write the same file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolStateEntry {
    pub version: String,
    pub installed_at: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ThirdPartyState {
    #[serde(flatten)]
    pub tools: HashMap<String, ToolStateEntry>,
}

pub fn load_state() -> ThirdPartyState {
    let path = paths::thirdparty_state_path();
    if path.exists() {
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    } else {
        ThirdPartyState::default()
    }
}

pub fn save_state(state: &ThirdPartyState) -> Result<(), String> {
    let json = serde_json::to_string_pretty(state)
        .map_err(|e| format!("Failed to serialize state: {e}"))?;
    paths::atomic_write(&paths::thirdparty_state_path(), &json)
}

pub fn now_iso() -> String {
    chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string()
}

/// Insert or update the entry for `tool_id`, keeping any other entries intact.
pub fn record_install(tool_id: &str, version: &str, enabled: bool) -> Result<(), String> {
    let mut state = load_state();
    state.tools.insert(
        tool_id.to_string(),
        ToolStateEntry {
            version: version.to_string(),
            installed_at: now_iso(),
            enabled,
        },
    );
    save_state(&state)
}

/// Set the `enabled` flag for `tool_id`, creating a placeholder entry when the
/// tool was deployed manually and has no state yet.
pub fn set_enabled(tool_id: &str, enabled: bool) -> Result<(), String> {
    let mut state = load_state();
    let entry = state
        .tools
        .entry(tool_id.to_string())
        .or_insert_with(|| ToolStateEntry {
            version: "detected".to_string(),
            installed_at: now_iso(),
            enabled: true,
        });
    entry.enabled = enabled;
    save_state(&state)
}
