use std::fs;
use std::path::{Path, PathBuf};

use crate::paths;

// ---------------------------------------------------------------------------
// Hooks run after a tool is installed (and re-run on enable)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum PostInstall {
    /// OpenSteamTool: `{steam}/config/lua` + seed/patch `{steam}/opensteamtool.toml`.
    SetupOst,
    /// CloudRedirect: set `[cloud] enabled = true` in `{steam}/opensteamtool.toml`.
    PatchOstToml,
    /// SLSsteam: patch steam.sh with LD_AUDIT, write steam.cfg, seed config.yaml.
    SlssteamSetup,
}

pub fn run_all(hooks: &[PostInstall], steam_root: &Path) -> Result<(), String> {
    for hook in hooks {
        run(*hook, steam_root)?;
    }
    Ok(())
}

pub fn run(hook: PostInstall, steam_root: &Path) -> Result<(), String> {
    match hook {
        PostInstall::SetupOst => setup_ost(steam_root),
        PostInstall::PatchOstToml => patch_opensteamtool_cloud_enabled(steam_root).map(|_| ()),
        PostInstall::SlssteamSetup => slssteam_setup(steam_root),
    }
}

/// Toggle-time counterpart of [`run_all`]: `SlssteamSetup` must be undone when
/// the tool is switched off (remove the LD_AUDIT line again).
pub fn on_toggle(hooks: &[PostInstall], enable: bool, steam_root: &Path) -> Result<(), String> {
    if enable {
        return run_all(hooks, steam_root);
    }
    if hooks.contains(&PostInstall::SlssteamSetup) {
        strip_ld_audit(steam_root)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// SetupOst (OpenSteamTool: config/lua + opensteamtool.toml)
// ---------------------------------------------------------------------------

/// The file we seed when `{steam}/opensteamtool.toml` does not exist yet.
const DEFAULT_OST_TOML: &str = "[cloud]\nenabled = true\n\n[manifest]\nurl = \"manifestdex\"\n";

fn setup_ost(steam_root: &Path) -> Result<(), String> {
    // OST loads its scripts from `{steam}/config/lua` (dllmain.cpp) — create
    // it recursively so a missing `config\` directory is fine too.
    let lua_dir = steam_root.join("config").join("lua");
    fs::create_dir_all(&lua_dir)
        .map_err(|e| format!("Failed to create {}: {e}", lua_dir.display()))?;

    ensure_ost_toml(steam_root)
}

/// Force `[cloud] enabled = true` and `[manifest] url = "manifestdex"` in
/// `{steam}/opensteamtool.toml`, creating the file when absent. Everything
/// else in an existing file (sections, comments, user values) is preserved.
fn ensure_ost_toml(steam_root: &Path) -> Result<(), String> {
    let toml_path = steam_root.join("opensteamtool.toml");

    if !toml_path.exists() {
        fs::write(&toml_path, DEFAULT_OST_TOML)
            .map_err(|e| format!("Failed to create opensteamtool.toml: {e}"))?;
        return Ok(());
    }

    let before = fs::read_to_string(&toml_path)
        .map_err(|e| format!("Failed to read opensteamtool.toml: {e}"))?;
    if before.contains("enabled = true")
        && before.contains("[manifest]")
        && before.contains("url = \"manifestdex\"")
    {
        return Ok(());
    }

    // One-shot backup before the first modification.
    let bak_path = toml_path.with_extension("toml.ost.bak");
    if !bak_path.exists() {
        let _ = fs::copy(&toml_path, &bak_path);
    }

    let newline = if before.contains("\r\n") { "\r\n" } else { "\n" };
    let mut lines: Vec<String> = before
        .split('\n')
        .map(|l| l.trim_end_matches('\r').to_string())
        .collect();

    force_toml_value(&mut lines, "cloud", "enabled", "true");
    force_toml_value(&mut lines, "manifest", "url", "manifestdex");

    fs::write(&toml_path, lines.join(newline))
        .map_err(|e| format!("Failed to write opensteamtool.toml: {e}"))
}

/// Ensure `<section>` has `key = "value"` (string values are always quoted;
/// `value` here is the raw text without quotes). Creates the section when it
/// is missing. Existing unrelated lines are untouched.
fn force_toml_value(lines: &mut Vec<String>, section: &str, key: &str, value: &str) {
    let header = format!("[{section}]");
    let expected = format!("{key} = \"{value}\"");

    // Locate the top-level section header (single `[x]`, not `[[x]]` or `[a.b]`).
    let mut section_idx: Option<usize> = None;
    for (i, line) in lines.iter().enumerate() {
        let t = line.trim();
        if t.starts_with('[') && !t.starts_with("[[") && t.ends_with(']') && !t.contains('.') {
            if t.eq_ignore_ascii_case(&header) {
                section_idx = Some(i);
                break;
            }
        }
    }

    let Some(start) = section_idx else {
        // Section absent → append it with the key.
        if !lines.is_empty() && !lines.last().map(|l| l.trim().is_empty()).unwrap_or(false) {
            lines.push(String::new());
        }
        lines.push(header);
        lines.push(expected);
        return;
    };

    // Find the end of the section (next top-level header).
    let end = lines
        .iter()
        .enumerate()
        .skip(start + 1)
        .find(|(_, line)| {
            let t = line.trim();
            t.starts_with('[') && !t.starts_with("[[") && t.ends_with(']') && !t.contains('.')
        })
        .map(|(i, _)| i)
        .unwrap_or(lines.len());

    // Replace an existing `key = …` inside the section, else insert after it.
    for i in (start + 1)..end {
        let t = lines[i].trim();
        if t.starts_with(key)
            && t[key.len()..].trim_start().starts_with('=')
            && t[..key.len()].eq_ignore_ascii_case(key)
        {
            if lines[i].trim() != expected {
                lines[i] = expected.clone();
            }
            return;
        }
    }
    lines.insert(start + 1, expected);
}

// ---------------------------------------------------------------------------
// PatchOstToml (ported from LumaForge utils/cloud_config.rs)
// ---------------------------------------------------------------------------

/// Set `[cloud].enabled = true` inside `{steam_root}/opensteamtool.toml`.
fn patch_opensteamtool_cloud_enabled(steam_root: &Path) -> Result<String, String> {
    let toml_path = steam_root.join("opensteamtool.toml");

    if !toml_path.exists() {
        fs::write(&toml_path, "[cloud]\nenabled = true\n")
            .map_err(|e| format!("Failed to create opensteamtool.toml: {e}"))?;
        return Ok("created".to_string());
    }

    let content = fs::read_to_string(&toml_path)
        .map_err(|e| format!("Failed to read opensteamtool.toml: {e}"))?;
    let newline = if content.contains("\r\n") { "\r\n" } else { "\n" };

    let lines: Vec<&str> = content.split('\n').collect();
    let mut cloud_section_found = false;
    let mut cloud_enabled_found = false;
    let mut cloud_enabled_value = false;
    let mut in_cloud_section = false;
    let mut cloud_section_line_idx = None;

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();

        if trimmed.starts_with('[')
            && !trimmed.starts_with("[[")
            && trimmed.ends_with(']')
            && !trimmed.contains('.')
        {
            if trimmed.eq_ignore_ascii_case("[cloud]") {
                in_cloud_section = true;
                cloud_section_found = true;
                cloud_section_line_idx = Some(i);
            } else {
                in_cloud_section = false;
            }
        }

        if in_cloud_section && trimmed.starts_with("enabled") {
            let parts: Vec<&str> = trimmed.splitn(2, '=').collect();
            if parts.len() == 2 && parts[0].trim().eq_ignore_ascii_case("enabled") {
                cloud_enabled_found = true;
                cloud_enabled_value = parts[1].trim().eq_ignore_ascii_case("true");
            }
        }
    }

    if cloud_section_found && !cloud_enabled_value {
        let bak_path = toml_path.with_extension("toml.cloudredirect.bak");
        let _ = fs::copy(&toml_path, &bak_path);
    }

    if cloud_section_found && cloud_enabled_found && cloud_enabled_value {
        return Ok("already_enabled".to_string());
    }

    if cloud_section_found && cloud_enabled_found && !cloud_enabled_value {
        let mut new_lines = Vec::new();
        let mut in_cloud = false;
        for line in &lines {
            let trimmed = line.trim();
            if trimmed.eq_ignore_ascii_case("[cloud]") {
                in_cloud = true;
                new_lines.push(line.to_string());
                continue;
            }
            if in_cloud && trimmed.starts_with("enabled") {
                let parts: Vec<&str> = trimmed.splitn(2, '=').collect();
                if parts.len() == 2 && parts[0].trim().eq_ignore_ascii_case("enabled") {
                    new_lines.push("enabled = true".to_string());
                    in_cloud = false;
                    continue;
                }
            }
            if in_cloud && trimmed.starts_with('[') && trimmed.ends_with(']') {
                in_cloud = false;
            }
            new_lines.push(line.to_string());
        }
        let new_content = new_lines.join(newline);
        fs::write(&toml_path, new_content)
            .map_err(|e| format!("Failed to write opensteamtool.toml: {e}"))?;
        return Ok("updated".to_string());
    }

    if !cloud_section_found {
        let mut new_content = content.trim_end().to_string();
        new_content.push_str(newline);
        new_content.push_str("[cloud]");
        new_content.push_str(newline);
        new_content.push_str("enabled = true");
        new_content.push_str(newline);
        fs::write(&toml_path, new_content)
            .map_err(|e| format!("Failed to write opensteamtool.toml: {e}"))?;
        return Ok("created".to_string());
    }

    if cloud_section_found && !cloud_enabled_found {
        let mut new_lines = Vec::new();
        let mut inserted = false;
        for (i, line) in lines.iter().enumerate() {
            new_lines.push(line.to_string());
            if !inserted && i == cloud_section_line_idx.unwrap_or(0) {
                new_lines.push("enabled = true".to_string());
                inserted = true;
            }
        }
        let new_content = new_lines.join(newline);
        fs::write(&toml_path, new_content)
            .map_err(|e| format!("Failed to write opensteamtool.toml: {e}"))?;
        return Ok("created".to_string());
    }

    Ok("already_enabled".to_string())
}

// ---------------------------------------------------------------------------
// SlssteamSetup (ported from LumaForge commands/slssteam.rs)
// ---------------------------------------------------------------------------

/// `bin/` first (the release 7z layout), then flat, then the well-known
/// `~/.local/share/SLSsteam` install locations.
fn find_slssteam_file(name: &str) -> Option<PathBuf> {
    let base = paths::thirdparty_dir().join("slssteam");
    let mut candidates = vec![base.join("bin").join(name), base.join(name)];
    if let Some(home) = dirs::home_dir() {
        candidates.push(home.join(".local/share/SLSsteam").join(name));
    }
    candidates.into_iter().find(|c| c.exists())
}

fn slssteam_setup(steam_root: &Path) -> Result<(), String> {
    let steam_sh = steam_root.join("steam.sh");
    if !steam_sh.exists() {
        return Err("steam.sh not found in the Steam root.".to_string());
    }

    let slssteam_so =
        find_slssteam_file("SLSsteam.so").ok_or_else(|| "SLSsteam.so not found in payload.".to_string())?;
    let library_inject = find_slssteam_file("library-inject.so")
        .ok_or_else(|| "library-inject.so not found in payload.".to_string())?;
    let ld_audit = format!("{}:{}", library_inject.display(), slssteam_so.display());

    let content = fs::read_to_string(&steam_sh)
        .map_err(|e| format!("Failed to read steam.sh: {e}"))?;

    let backup = steam_root.join("steam.sh.bak");
    if !backup.exists() {
        fs::copy(&steam_sh, &backup)
            .map_err(|e| format!("Failed to create steam.sh.bak: {e}"))?;
    }

    write_patched_steam_sh(&steam_sh, &content, Some(&ld_audit))?;

    // Block client self-updates so the LD_AUDIT patch survives.
    fs::write(
        steam_root.join("steam.cfg"),
        "BootStrapperInhibitAll=enable\nBootStrapperForceSelfUpdate=disable\n",
    )
    .map_err(|e| format!("Failed to create steam.cfg: {e}"))?;

    // Seed a default config only when none exists — never clobber user data.
    let config_path = slssteam_config_path();
    if !config_path.exists() {
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create {}: {e}", parent.display()))?;
        }
        fs::write(&config_path, DEFAULT_SLSSTEAM_CONFIG)
            .map_err(|e| format!("Failed to create config.yaml: {e}"))?;
    }

    Ok(())
}

fn slssteam_config_path() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        PathBuf::from(xdg).join("SLSsteam").join("config.yaml")
    } else {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".config")
            .join("SLSsteam")
            .join("config.yaml")
    }
}

/// Rewrite steam.sh dropping any previous `export LD_AUDIT=` line and
/// inserting the given one after line 10 (`None` = just strip).
fn write_patched_steam_sh(path: &Path, content: &str, ld_audit: Option<&str>) -> Result<(), String> {
    let lines: Vec<&str> = content.lines().collect();
    let mut new_lines: Vec<String> = Vec::new();
    let mut inserted = false;

    for (i, line) in lines.iter().enumerate() {
        if line.contains("export LD_AUDIT=") {
            continue;
        }
        new_lines.push(line.to_string());

        if let Some(ld) = ld_audit {
            if !inserted && i == 9 {
                new_lines.push(format!("export LD_AUDIT={ld}"));
                inserted = true;
            }
        }
    }

    if let Some(ld) = ld_audit {
        if !inserted {
            new_lines.push(format!("export LD_AUDIT={ld}"));
        }
    }

    fs::write(path, new_lines.join("\n"))
        .map_err(|e| format!("Failed to write steam.sh: {e}"))
}

fn strip_ld_audit(steam_root: &Path) -> Result<(), String> {
    let steam_sh = steam_root.join("steam.sh");
    if !steam_sh.exists() {
        return Ok(());
    }
    let content = fs::read_to_string(&steam_sh)
        .map_err(|e| format!("Failed to read steam.sh: {e}"))?;
    if !content.contains("export LD_AUDIT=") {
        return Ok(());
    }
    write_patched_steam_sh(&steam_sh, &content, None)
}

const DEFAULT_SLSSTEAM_CONFIG: &str = r#"# SLSsteam Configuration
# Generated by LumaForge Panel

DisableFamilyShareLock: yes
UseWhitelist: no
AppIds:
AdditionalApps:
DlcData:
AppTokens:
CDKeys:
FakeOffline:
FakeAppIds:
ManifestIds:
DepotBlacklist:
GameTitles:
SubscriptionTimestamps:
DenuvoGames:
SteamIdOverride:
SmartTickets: 0x1
MaxSchemaTries: 10
LaunchOptions:
SafeMode: no
WarnHashMissmatch: no
NotifyInit: yes
API: yes
Plugins: no
DisableCloud: yes
DisableUpdates: yes
FakeName: ""
FakeEmail: ""
FakeWalletBalance: 0
LogLevels: 0xff
DumpClientInterfaces: no
ExtendedLogging: no
"#;
