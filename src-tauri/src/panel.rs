use serde::Serialize;

use crate::{github, paths, settings, state, steam, tools};

// ---------------------------------------------------------------------------
// Types surfaced to the frontend
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolStatus {
    pub id: String,
    pub name: String,
    pub description: String,
    pub state: String,
    pub installed: bool,
    pub installed_version: Option<String>,
    pub latest_version: Option<String>,
    pub update_available: bool,
    pub release_available: bool,
    pub deploy_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PanelStatus {
    pub steam_root: Option<String>,
    pub steam: steam::SteamStatus,
    pub cdp: String,
    /// Primary file the CDP toggle renames — `wsock32.dll` on Windows,
    /// `ubuntu12_32/liblumaforge.so` on Linux.
    pub cdp_loader: String,
    pub tools: Vec<ToolStatus>,
    pub runtime_installed: bool,
    pub runtime_update_available: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PanelResult {
    pub ok: bool,
    pub message: String,
    pub restart_required: bool,
}

fn ok(message: impl Into<String>, restart_required: bool) -> PanelResult {
    PanelResult {
        ok: true,
        message: message.into(),
        restart_required,
    }
}

fn err(message: impl Into<String>) -> PanelResult {
    PanelResult {
        ok: false,
        message: message.into(),
        restart_required: false,
    }
}

// ---------------------------------------------------------------------------
// Status
// ---------------------------------------------------------------------------

fn build_status() -> PanelStatus {
    let steam_root = paths::detect_steam_root();
    let steam_status = steam::current_status();
    let runtime = tools::find_tool("cdp-proxy");
    let plugin = tools::find_tool("steam-store-helper");

    let state_map = state::load_state();

    let mut tool_statuses = Vec::new();

    for def in tools::TOOL_DEFS {
        if !def.platform.matches() {
            continue;
        }
        let component = tools::tool_state(def);
        let payload_installed = tools::payload_installed(def);

        let entry = state_map.tools.get(def.id);
        let installed_version = entry.map(|e| e.version.clone());

        let (owner, repo, asset, contains) = (
            def.github_owner,
            def.github_repo,
            def.preferred_asset,
            def.preferred_asset_contains,
        );

        let release = github::get_latest_github_release(owner, repo, asset, contains);
        let (latest_version, release_available) = match &release {
            Ok(info) => (Some(info.tag_name.clone()), true),
            Err(_) => (None, false),
        };

        let update_available = match (&installed_version, &latest_version) {
            (Some(installed), Some(latest)) => {
                installed != latest && installed != "detected"
            }
            _ => false,
        };

        let deploy_path = match def.target {
            tools::DeployTarget::SteamRoot { .. } => steam_root
                .as_ref()
                .map(|root| root.to_string_lossy().to_string()),
            tools::DeployTarget::Plugins { .. } => {
                Some(paths::plugins_dir().to_string_lossy().to_string())
            }
            tools::DeployTarget::Payload => {
                Some(tools::payload_dir(def).to_string_lossy().to_string())
            }
        };

        tool_statuses.push(ToolStatus {
            id: def.id.to_string(),
            name: def.name.to_string(),
            description: def.description.to_string(),
            state: component.as_str().to_string(),
            installed: payload_installed || component != tools::ComponentState::Missing,
            installed_version,
            latest_version,
            update_available,
            release_available,
            deploy_path,
        });
    }

    let cdp_state = runtime
        .map(tools::tool_state)
        .unwrap_or(tools::ComponentState::Missing);

    let runtime_installed = cdp_state != tools::ComponentState::Missing
        || plugin.map(tools::tool_state).unwrap_or(tools::ComponentState::Missing)
            != tools::ComponentState::Missing;

    let runtime_update_available = tool_statuses
        .iter()
        .filter(|t| t.id == "cdp-proxy" || t.id == "steam-store-helper")
        .any(|t| t.update_available);

    PanelStatus {
        steam_root: steam_root.map(|p| p.to_string_lossy().to_string()),
        steam: steam_status,
        cdp: cdp_state.as_str().to_string(),
        cdp_loader: runtime
            .and_then(|def| def.toggle.first().copied())
            .unwrap_or_default()
            .to_string(),
        tools: tool_statuses,
        runtime_installed,
        runtime_update_available,
    }
}

fn status_blocking() -> Result<PanelStatus, String> {
    Ok(build_status())
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn panel_status() -> Result<PanelStatus, String> {
    tauri::async_runtime::spawn_blocking(status_blocking)
        .await
        .map_err(|e| crate::i18n::t("err.status_task", &[&e.to_string()]))?
}

/// Clear the GitHub cache and re-read the latest releases.
#[tauri::command]
pub async fn check_updates() -> Result<PanelStatus, String> {
    tauri::async_runtime::spawn_blocking(|| {
        github::invalidate_cache();
        build_status()
    })
    .await
    .map_err(|e| crate::i18n::t("err.update_check", &[&e.to_string()]))
}

/// Big CDP switch: rename `wsock32.dll` <-> `wsock32.dll.bak`.
#[tauri::command]
pub async fn cdp_set_enabled(
    app: tauri::AppHandle,
    enabled: bool,
) -> Result<PanelResult, String> {
    let result = tauri::async_runtime::spawn_blocking(move || {
        let Some(def) = tools::find_tool("cdp-proxy") else {
            return Err(crate::i18n::t("err.cdp_def_missing", &[]));
        };
        tools::set_enabled(&app, def, enabled)
    })
    .await
    .map_err(|e| crate::i18n::t("err.cdp_toggle", &[&e.to_string()]))?;

    match result {
        Ok((message, restart_required)) => Ok(ok(message, restart_required)),
        Err(e) => Ok(err(e)),
    }
}

/// Switch for steam-store-helper / OpenSteamTool / CloudRedirect.
/// First ON downloads and deploys the release.
#[tauri::command]
pub async fn set_tool_enabled(
    app: tauri::AppHandle,
    tool_id: String,
    enabled: bool,
) -> Result<PanelResult, String> {
    let result = tauri::async_runtime::spawn_blocking(move || {
        let def = tools::find_tool(&tool_id)
            .ok_or_else(|| crate::i18n::t("err.unknown_tool", &[&tool_id]))?;
        tools::set_enabled(&app, def, enabled)
    })
    .await
    .map_err(|e| crate::i18n::t("err.tool_toggle", &[&e.to_string()]))?;

    match result {
        Ok((message, restart_required)) => Ok(ok(message, restart_required)),
        Err(e) => Ok(err(e)),
    }
}

/// Install/Update banner: CDP proxy runtime + steam-store-helper plugin.
#[tauri::command]
pub async fn install_runtime(app: tauri::AppHandle) -> Result<PanelResult, String> {
    let result = tauri::async_runtime::spawn_blocking(move || {
        let mut messages: Vec<String> = Vec::new();
        let mut restart_required = false;
        let mut first_error: Option<String> = None;

        for id in ["cdp-proxy", "steam-store-helper"] {
            let Some(def) = tools::find_tool(id) else {
                continue;
            };
            match tools::install(&app, def, true) {
                Ok((message, restart)) => {
                    messages.push(message);
                    restart_required |= restart;
                }
                Err(e) => {
                    if first_error.is_none() {
                        first_error = Some(e);
                    }
                }
            }
        }

        match first_error {
            Some(e) => Err(format!("{} ({})", messages.join(" "), e)),
            None => Ok((messages.join(" "), restart_required)),
        }
    })
    .await
    .map_err(|e| crate::i18n::t("err.runtime_install", &[&e.to_string()]))?;

    match result {
        Ok((message, restart_required)) => Ok(ok(message, restart_required)),
        Err(e) => Ok(err(e)),
    }
}

#[tauri::command]
pub async fn get_steam_status() -> Result<steam::SteamStatus, String> {
    tauri::async_runtime::spawn_blocking(steam::current_status)
        .await
        .map_err(|e| crate::i18n::t("err.steam_status_task", &[&e.to_string()]))
}

#[tauri::command]
pub async fn start_steam() -> Result<steam::SteamOpResult, String> {
    tauri::async_runtime::spawn_blocking(steam::start_steam)
        .await
        .map_err(|e| crate::i18n::t("err.steam_start_task", &[&e.to_string()]))
}

#[tauri::command]
pub async fn restart_steam() -> Result<steam::SteamOpResult, String> {
    tauri::async_runtime::spawn_blocking(steam::restart_steam)
        .await
        .map_err(|e| crate::i18n::t("err.steam_restart_task", &[&e.to_string()]))
}

#[tauri::command]
pub fn get_app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Reveal the LumaForge data directory (`{local_data}/LumaForge`) in the
/// file manager — the folder button in the titlebar.
#[tauri::command]
pub fn open_app_data_dir() -> Result<(), String> {
    let dir = paths::app_data_dir();
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("{}", crate::i18n::t("err.create_dir", &[&dir.display().to_string(), &e.to_string()])))?;

    #[cfg(target_os = "windows")]
    let status = std::process::Command::new("explorer")
        .arg(&dir)
        .spawn();

    #[cfg(target_os = "linux")]
    let status = std::process::Command::new("xdg-open")
        .arg(&dir)
        .spawn();

    status.map(|_| ()).map_err(|e| format!("{}", crate::i18n::t("err.open", &[&dir.display().to_string(), &e.to_string()])))
}

// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn get_settings() -> settings::PanelSettings {
    settings::get_settings()
}

#[tauri::command]
pub fn update_settings(
    partial: settings::PartialSettings,
) -> Result<settings::PanelSettings, String> {
    settings::update_settings(partial)
}

// ---------------------------------------------------------------------------
// Install / Update
// ---------------------------------------------------------------------------

/// Force-install (or update) a single tool from its latest GitHub release.
#[tauri::command]
pub async fn install_tool(app: tauri::AppHandle, tool_id: String) -> Result<PanelResult, String> {
    let result = tauri::async_runtime::spawn_blocking(move || {
        let def = tools::find_tool(&tool_id)
            .ok_or_else(|| crate::i18n::t("err.unknown_tool", &[&tool_id]))?;
        tools::install(&app, def, true)
    })
    .await
    .map_err(|e| crate::i18n::t("err.install", &[&e.to_string()]))?;

    match result {
        Ok((message, restart_required)) => Ok(ok(message, restart_required)),
        Err(e) => Ok(err(e)),
    }
}

/// Remove everything a tool deployed (files, state entry) — the per-row
/// uninstall button on the dashboard.
#[tauri::command]
pub async fn uninstall_tool(tool_id: String) -> Result<PanelResult, String> {
    let result = tauri::async_runtime::spawn_blocking(move || {
        let def = tools::find_tool(&tool_id)
            .ok_or_else(|| crate::i18n::t("err.unknown_tool", &[&tool_id]))?;
        tools::uninstall(def)
    })
    .await
    .map_err(|e| crate::i18n::t("err.uninstall", &[&e.to_string()]))?;

    match result {
        Ok((message, restart_required)) => Ok(ok(message, restart_required)),
        Err(e) => Ok(err(e)),
    }
}
