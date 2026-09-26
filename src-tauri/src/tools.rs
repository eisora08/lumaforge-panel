use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri::Emitter;

use crate::{github, paths, postinstall, state, steam, postinstall::PostInstall};

// ---------------------------------------------------------------------------
// Definitions
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ComponentState {
    Enabled,
    Disabled,
    Missing,
}

impl ComponentState {
    pub fn as_str(self) -> &'static str {
        match self {
            ComponentState::Enabled => "enabled",
            ComponentState::Disabled => "disabled",
            ComponentState::Missing => "missing",
        }
    }
}

/// OS availability of a tool — tools that only exist on one OS are hidden
/// (and refused) on the others.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Platform {
    All,
    Windows,
    Linux,
}

impl Platform {
    pub fn matches(self) -> bool {
        match self {
            Platform::All => true,
            Platform::Windows => cfg!(target_os = "windows"),
            Platform::Linux => cfg!(target_os = "linux"),
        }
    }
}

const IS_LINUX: bool = cfg!(target_os = "linux");

pub enum DeployTarget {
    /// Copy the listed relative paths into the Steam root.
    SteamRoot { files: &'static [&'static str] },
    /// Copy the whole payload into `{plugins}/{dir}`.
    Plugins { dir: &'static str },
    /// Keep everything in `thirdparty/{id}` — the tool is wired up through
    /// config files instead of deployed files (SLSsteam on Linux).
    Payload,
}

pub struct ToolDef {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub github_owner: &'static str,
    pub github_repo: &'static str,
    pub preferred_asset: Option<&'static str>,
    pub preferred_asset_contains: Option<&'static str>,
    pub platform: Platform,
    pub target: DeployTarget,
    /// Relative paths flipped between `x` and `x.bak` on toggle.
    pub toggle: &'static [&'static str],
    /// Evidence that the tool is installed (live or backed up) even when no
    /// `toggle` path exists — e.g. `lumaforge\` without the `wsock32.dll` loader.
    pub installed: &'static [&'static str],
    /// Idempotent steps executed after a successful install (and on enable).
    pub post_install: &'static [PostInstall],
}

impl ToolDef {
    fn resolve_github(&self) -> (&str, &str, Option<&str>, Option<&str>) {
        (
            self.github_owner,
            self.github_repo,
            self.preferred_asset,
            self.preferred_asset_contains,
        )
    }

    fn root(&self, steam_root: &Path) -> Option<PathBuf> {
        match self.target {
            DeployTarget::SteamRoot { .. } => Some(steam_root.to_path_buf()),
            DeployTarget::Plugins { .. } => Some(paths::plugins_dir()),
            DeployTarget::Payload => Some(payload_dir(self)),
        }
    }

    }

// Per-OS deploy layouts. The CDP proxy Linux build ships `libXtst.so.6` +
// `liblumaforge.so` inside `ubuntu12_32/`; Windows ships `wsock32.dll` +
// the `lumaforge\` runtime directory.
const CDP_FILES: &[&str] = if IS_LINUX {
    &["ubuntu12_32/libXtst.so.6", "ubuntu12_32/liblumaforge.so"]
} else {
    &["wsock32.dll", "lumaforge"]
};

// Only `liblumaforge.so` doubles as state marker on Linux: Steam ships its
// own `libXtst.so.6`, so that file must never be used to decide installed-ness.
const CDP_TOGGLE: &[&str] = if IS_LINUX {
    &["ubuntu12_32/liblumaforge.so"]
} else {
    &["wsock32.dll"]
};

const CDP_INSTALLED: &[&str] = if IS_LINUX {
    &["ubuntu12_32/liblumaforge.so"]
} else {
    &["lumaforge/lumaforge.dll", "lumaforge/lumaforge_cef_hook.dll"]
};

const CLOUD_REDIRECT_ASSET: Option<&str> = if IS_LINUX {
    Some("cloud_redirect.so")
} else {
    Some("cloud_redirect.dll")
};

const CLOUD_REDIRECT_FILES: &[&str] = if IS_LINUX {
    &["cloud_redirect.so"]
} else {
    &["cloud_redirect.dll"]
};

pub const TOOL_DEFS: &[ToolDef] = &[
    ToolDef {
        id: "cdp-proxy",
        name: "CDP Proxy",
        description: "wsock32 bootstrap loader + lumaforge DLLs — injects extensions into Steam.",
        github_owner: "eisora08",
        github_repo: "lumaforge-cdp-proxy",
        preferred_asset: None,
        preferred_asset_contains: None,
        platform: Platform::All,
        target: DeployTarget::SteamRoot { files: CDP_FILES },
        toggle: CDP_TOGGLE,
        installed: CDP_INSTALLED,
        post_install: &[],
    },
    ToolDef {
        id: "steam-store-helper",
        name: "steam-store-helper",
        description:
            "Store detection, manifest downloads and library folder management.",
        github_owner: "eisora08",
        github_repo: "lumaforge-extensions",
        preferred_asset: None,
        preferred_asset_contains: Some("steam-store-helper"),
        platform: Platform::All,
        target: DeployTarget::Plugins {
            dir: "steam-store-helper",
        },
        toggle: &["steam-store-helper"],
        installed: &["steam-store-helper"],
        post_install: &[],
    },
    ToolDef {
        id: "opensteamtool",
        name: "OpenSteamTool",
        description: "Open-source Steam unlocker with Lua scripting.",
        github_owner: "eisora08",
        github_repo: "OpenSteamTool",
        preferred_asset: None,
        preferred_asset_contains: Some("Release"),
        platform: Platform::Windows,
        target: DeployTarget::SteamRoot {
            files: &["dwmapi.dll", "xinput1_4.dll", "OpenSteamTool.dll"],
        },
        toggle: &["dwmapi.dll", "xinput1_4.dll", "OpenSteamTool.dll"],
        installed: &["dwmapi.dll", "xinput1_4.dll", "OpenSteamTool.dll"],
        post_install: &[PostInstall::SetupOst],
    },
    ToolDef {
        id: "cloud_redirect",
        name: "CloudRedirect",
        description: "Redirects Steam Cloud saves to Google Drive, OneDrive, S3, R2 or local.",
        github_owner: "Selectively11",
        github_repo: "CloudRedirect",
        preferred_asset: CLOUD_REDIRECT_ASSET,
        preferred_asset_contains: None,
        platform: Platform::All,
        target: DeployTarget::SteamRoot {
            files: CLOUD_REDIRECT_FILES,
        },
        toggle: CLOUD_REDIRECT_FILES,
        installed: CLOUD_REDIRECT_FILES,
        post_install: &[PostInstall::PatchOstToml],
    },
    ToolDef {
        id: "slssteam",
        name: "SLS Steam",
        description: "LD_AUDIT Steam unlocker for Linux — keeps the stock client, unlocks the rest.",
        github_owner: "AceSLS",
        github_repo: "SLSsteam",
        preferred_asset: None,
        preferred_asset_contains: Some("SLSsteam-Any-release"),
        platform: Platform::Linux,
        target: DeployTarget::Payload,
        toggle: &["bin/SLSsteam.so", "bin/library-inject.so"],
        installed: &[],
        post_install: &[PostInstall::SlssteamSetup],
    },
];

pub fn find_tool(id: &str) -> Option<&'static ToolDef> {
    TOOL_DEFS.iter().find(|def| def.id == id)
}

// ---------------------------------------------------------------------------
// Disk inspection
// ---------------------------------------------------------------------------

/// Resolve a declared relative path against `root`, tolerating archives that
/// extracted the file flat instead of into its declared subdirectory
/// (`bin/SLSsteam.so` vs `SLSsteam.so`).
fn resolve_rel(root: &Path, rel: &str) -> PathBuf {
    let direct = root.join(rel);
    if direct.exists() || PathBuf::from(format!("{}.bak", direct.display())).exists() {
        return direct;
    }
    if rel.contains('/') {
        if let Some(base) = rel.rsplit('/').next() {
            let flat = root.join(base);
            if flat.exists() || PathBuf::from(format!("{}.bak", flat.display())).exists() {
                return flat;
            }
        }
    }
    direct
}

fn component_state(root: &Path, rel_paths: &[&str]) -> ComponentState {
    if rel_paths.is_empty() {
        return ComponentState::Missing;
    }

    let mut live = 0usize;
    let mut backed_up = 0usize;

    for rel in rel_paths {
        let path = resolve_rel(root, rel);
        if path.exists() {
            live += 1;
        } else if PathBuf::from(format!("{}.bak", path.display())).exists() {
            backed_up += 1;
        }
    }

    if live > 0 {
        ComponentState::Enabled
    } else if backed_up > 0 {
        ComponentState::Disabled
    } else {
        ComponentState::Missing
    }
}

/// State of a tool. Every installed marker must be on disk (live or backed
/// up); if any is missing the tool is a broken/partial install and reports
/// `Missing` so the INSTALL button can repair it — e.g. `wsock32.dll` alone
/// without the `lumaforge\` DLLs is not "installed".
pub fn tool_state(def: &ToolDef) -> ComponentState {
    let steam_root = paths::detect_steam_root().unwrap_or_default();
    let Some(root) = def.root(&steam_root) else {
        return ComponentState::Missing;
    };

    if !def.installed.is_empty()
        && def.installed.iter().any(|rel| {
            let path = resolve_rel(&root, rel);
            !path.exists() && !PathBuf::from(format!("{}.bak", path.display())).exists()
        })
    {
        return ComponentState::Missing;
    }

    component_state(&root, def.toggle)
}

/// Only `Payload` tools keep their files in `thirdparty/{id}` — Steam-root and
/// plugin tools deploy straight to their target and leave nothing behind.
pub fn payload_dir(def: &ToolDef) -> PathBuf {
    paths::thirdparty_dir().join(def.id)
}

pub fn payload_installed(def: &ToolDef) -> bool {
    matches!(def.target, DeployTarget::Payload)
        && github::is_dir_populated(&payload_dir(def))
}

// ---------------------------------------------------------------------------
// Rename .bak toggling
// ---------------------------------------------------------------------------

fn rename_one(from: &Path, to: &Path) -> Result<(), std::io::Error> {
    if to.exists() {
        std::fs::remove_file(to).or_else(|_| std::fs::remove_dir_all(to))?;
    }
    std::fs::rename(from, to)
}

/// Flip `rel` between live and `.bak` inside `root`.
fn flip_path(root: &Path, rel: &str, enable: bool) -> Result<(), String> {
    let live = resolve_rel(root, rel);
    let backup = PathBuf::from(format!("{}.bak", live.display()));

    if enable {
        if live.exists() || !backup.exists() {
            return Ok(());
        }
    } else {
        if !live.exists() {
            return Ok(());
        }
    }

    let (from, to) = if enable {
        (&backup, &live)
    } else {
        (&live, &backup)
    };

    rename_one(from, to).map_err(|e| {
        format!(
            "Failed to rename {} -> {}: {e}",
            from.display(),
            to.display()
        )
    })
}

fn flip_all(root: &Path, rels: &[&str], enable: bool) -> Result<(), String> {
    for rel in rels {
        flip_path(root, rel, enable)?;
    }
    Ok(())
}

/// Generic toggle used by every switch on the dashboard.
/// Returns `(message, restart_required)`.
pub fn set_enabled(app: &tauri::AppHandle, def: &ToolDef, enable: bool) -> Result<(String, bool), String> {
    let result = set_enabled_inner(app, def, enable);
    if let Err(error) = &result {
        log_line("error", &format!("{} toggle({enable}) failed: {error}", def.name));
    }
    result
}

fn set_enabled_inner(app: &tauri::AppHandle, def: &ToolDef, enable: bool) -> Result<(String, bool), String> {
    if !def.platform.matches() {
        return Err(format!("{} is not available on this OS.", def.name));
    }

    let current = tool_state(def);

    // First ON (or evidence/payload-only ON): make sure the files are on
    // disk — downloads when there is no payload, re-deploys when there is.
    if enable && current == ComponentState::Missing {
        return install(app, def, false);
    }

    let steam_root = paths::detect_steam_root()
        .ok_or_else(|| "Steam root not found.".to_string())?;
    let Some(root) = def.root(&steam_root) else {
        return Err("Deploy root not found.".to_string());
    };

    let mut restart_required = false;
    let is_steam_root_deploy = matches!(def.target, DeployTarget::SteamRoot { .. });

    if is_steam_root_deploy && steam::is_steam_running() {
        steam::ensure_steam_stopped()?;
        restart_required = true;
    }

    flip_all(&root, def.toggle, enable)?;
    if !def.post_install.is_empty() {
        postinstall::on_toggle(def.post_install, enable, &steam_root)?;
    }
    state::set_enabled(def.id, enable)?;

    let message = if enable {
        format!("{} enabled.", def.name)
    } else {
        format!("{} disabled.", def.name)
    };

    Ok((message, restart_required))
}

// ---------------------------------------------------------------------------
// Install
// ---------------------------------------------------------------------------

/// Append a line to `%TEMP%\panel.log` so install failures are never silent.
fn log_line(step: &str, message: &str) {
    use std::io::Write;

    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let line = format!("[{ms}] [{step}] {message}\n");

    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(paths::temp_dir().join("panel.log"))
    {
        let _ = file.write_all(line.as_bytes());
    }
}

fn emit_progress(app: &tauri::AppHandle, step: &str, message: impl Into<String>) {
    #[derive(Clone, Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Progress {
        step: String,
        message: String,
    }

    let message = message.into();
    log_line(step, &message);
    let _ = app.emit(
        "panel://progress",
        Progress {
            step: step.to_string(),
            message,
        },
    );
}

fn copy_entry(src: &Path, dst: &Path) -> Result<(), String> {
    if src.is_dir() {
        if dst.exists() {
            github::remove_dir_recursive(dst)?;
        }
        github::copy_dir_recursive(src, dst)?;
    } else {
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::copy(src, dst).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Deploy the extracted payload (rooted at `src`) to its target (Steam root
/// or plugins dir). Stops Steam first when a Steam-root file is in use.
/// Every declared file must be present — anything short of that is an error.
fn deploy(app: &tauri::AppHandle, def: &ToolDef, src: &Path) -> Result<bool, String> {
    match def.target {
        DeployTarget::Payload => {
            // Everything lives in the payload dir; wiring happens in the
            // post-install hooks (steam.sh / config files).
            Ok(false)
        }
        DeployTarget::Plugins { dir } => {
            emit_progress(app, "deploy", "Copying plugin files...");
            let dst = paths::plugins_dir().join(dir);
            if dst.exists() {
                github::remove_dir_recursive(&dst)?;
            }
            std::fs::create_dir_all(&dst).map_err(|e| e.to_string())?;
            github::copy_dir_recursive(src, &dst)?;
            Ok(false)
        }
        DeployTarget::SteamRoot { files } => {
            let steam_root = paths::detect_steam_root()
                .ok_or_else(|| "Steam root not found.".to_string())?;
            emit_progress(app, "deploy", "Deploying files to the Steam directory...");

            let mut restart_required = false;
            let mut errors: Vec<String> = Vec::new();

            let copy_once = |rel: &str| -> Result<(), String> {
                let from = src.join(rel);
                if !from.exists() {
                    return Err(format!("{rel} not found in release"));
                }
                copy_entry(&from, &steam_root.join(rel))
            };

            for rel in files {
                if let Err(e) = copy_once(rel) {
                    // Locked DLL: stop Steam, then retry once.
                    if steam::is_steam_running() {
                        steam::ensure_steam_stopped()?;
                        restart_required = true;
                        if let Err(e2) = copy_once(rel) {
                            errors.push(e2);
                        }
                    } else {
                        errors.push(e);
                    }
                }
            }

            // The deploy is only complete when every installed marker made it
            // to disk — catches archives shipped without the expected files
            // instead of leaving a half-install that looks fine.
            if errors.is_empty() {
                for rel in def.installed {
                    let path = resolve_rel(&steam_root, rel);
                    if !path.exists()
                        && !PathBuf::from(format!("{}.bak", path.display())).exists()
                    {
                        errors.push(format!("{rel} missing after deploy"));
                    }
                }
            }

            if !errors.is_empty() {
                return Err(format!(
                    "Deploy of {} failed: {}",
                    def.name,
                    errors.join("; ")
                ));
            }

            Ok(restart_required)
        }
    }
}

/// Run the tool's post-install hooks against the detected Steam root.
fn run_post_install(app: &tauri::AppHandle, def: &ToolDef) -> Result<(), String> {
    if def.post_install.is_empty() {
        return Ok(());
    }
    let steam_root = paths::detect_steam_root()
        .ok_or_else(|| "Steam root not found - cannot finish setup.".to_string())?;
    emit_progress(app, "setup", "Running post-install setup...");
    postinstall::run_all(def.post_install, &steam_root)
}

/// Download the latest release, extract it into a temp dir and deploy from
/// there. Only `Payload` tools persist their files (`thirdparty/{id}`);
/// everything else deploys straight to the target and leaves no staging.
/// Returns `(message, restart_required)`.
pub fn install(
    app: &tauri::AppHandle,
    def: &ToolDef,
    force: bool,
) -> Result<(String, bool), String> {
    let result = install_inner(app, def, force);
    if let Err(error) = &result {
        log_line("error", &format!("{} install failed: {error}", def.name));
    }
    result
}

fn install_inner(
    app: &tauri::AppHandle,
    def: &ToolDef,
    force: bool,
) -> Result<(String, bool), String> {
    if !def.platform.matches() {
        return Err(format!("{} is not available on this OS.", def.name));
    }

    let is_payload = matches!(def.target, DeployTarget::Payload);
    let target_dir = payload_dir(def);

    // Reuse an already-persisted payload (Payload tools only).
    if is_payload && !force && github::is_dir_populated(&target_dir) {
        let state = state::load_state();
        if !state.tools.contains_key(def.id) {
            let (owner, repo, asset, contains) = def.resolve_github();
            let version = github::get_latest_github_release(owner, repo, asset, contains)
                .map(|r| r.tag_name)
                .unwrap_or_else(|_| "detected".to_string());
            let _ = state::record_install(def.id, &version, true);
        }

        emit_progress(app, "deploy", "Files already present - deploying...");
        let restart_required = deploy(app, def, &target_dir)?;
        run_post_install(app, def)?;
        return Ok((format!("{} is already installed.", def.name), restart_required));
    }

    emit_progress(app, "fetch", format!("Fetching {} release...", def.name));
    let (owner, repo, asset, contains) = def.resolve_github();
    let release = github::get_latest_github_release(owner, repo, asset, contains)
        .map_err(|e| {
            format!(
                "No release available for {} yet: {e}",
                def.name
            )
        })?;

    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let temp_dir = paths::temp_dir().join(format!("panel_{}_{}", def.id, ts));
    std::fs::create_dir_all(&temp_dir).map_err(|e| e.to_string())?;

    let outcome = (|| -> Result<(String, bool), String> {
        emit_progress(
            app,
            "download",
            format!("Downloading {} v{}...", def.name, release.tag_name),
        );
        let archive_path = temp_dir.join(&release.zip_name);
        github::download_file(&release.zip_url, &archive_path)
            .map_err(|e| format!("Download failed: {e}"))?;

        emit_progress(app, "extract", "Extracting...");
        let effective_src = if release.archive_ext == "dll" || release.archive_ext == "so" {
            // The archive *is* the deployable file, already sitting at
            // `temp_dir/{zip_name}` — which matches the deploy rel path.
            temp_dir.clone()
        } else {
            let extract_dir = temp_dir.join("extracted");
            github::extract_archive(&archive_path, &release.archive_ext, &extract_dir)
                .map_err(|e| format!("Extraction failed: {e}"))?;
            github::flatten_extracted_dir(&extract_dir).unwrap_or(extract_dir)
        };

        // Payload tools persist their files; everyone else deploys straight
        // from the temp dir so `thirdparty/{id}` never gets created for them.
        if is_payload {
            std::fs::create_dir_all(&target_dir)
                .map_err(|e| format!("Failed to create tool dir: {e}"))?;
            github::copy_dir_recursive(&effective_src, &target_dir)?;
        }

        let restart_required = deploy(app, def, &effective_src)?;
        run_post_install(app, def)?;
        state::record_install(def.id, &release.tag_name, true)?;
        let _ = std::fs::remove_dir_all(&temp_dir);

        Ok((
            format!("{} v{} installed.", def.name, release.tag_name),
            restart_required,
        ))
    })();

    match outcome {
        Ok(result) => Ok(result),
        Err(error) => {
            let _ = std::fs::remove_dir_all(&temp_dir);
            Err(error)
        }
    }
}

