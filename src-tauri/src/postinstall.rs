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
    /// CloudRedirect (Windows): set `[cloud] enabled = true` in
    /// `{steam}/opensteamtool.toml`.
    PatchOstToml,
    /// SLSsteam: patch steam.sh with LD_AUDIT, write steam.cfg, seed config.yaml.
    SlssteamSetup,
    /// CloudRedirect (Linux): require SLSsteam, flip `DisableCloud: no` in its
    /// config and add cloud_redirect.so to the steam.sh LD_AUDIT chain.
    CloudRedirectSetup,
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
        PostInstall::CloudRedirectSetup => cloud_redirect_setup(steam_root),
    }
}

/// Toggle-time counterpart of [`run_all`]: the LD_AUDIT chain must be
/// rebuilt when a tool that participates in it is switched off (its files
/// are already `.bak` by the time this runs).
pub fn on_toggle(hooks: &[PostInstall], enable: bool, steam_root: &Path) -> Result<(), String> {
    if enable {
        return run_all(hooks, steam_root);
    }
    if hooks.contains(&PostInstall::SlssteamSetup)
        || hooks.contains(&PostInstall::CloudRedirectSetup)
    {
        sync_ld_audit(steam_root)?;
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
        .map_err(|e| format!("{}", crate::i18n::t("err.create_dir", &[&lua_dir.display().to_string(), &e.to_string()])))?;

    ensure_ost_toml(steam_root)
}

/// Force `[cloud] enabled = true` and `[manifest] url = "manifestdex"` in
/// `{steam}/opensteamtool.toml`, creating the file when absent. Everything
/// else in an existing file (sections, comments, user values) is preserved.
fn ensure_ost_toml(steam_root: &Path) -> Result<(), String> {
    let toml_path = steam_root.join("opensteamtool.toml");

    if !toml_path.exists() {
        fs::write(&toml_path, DEFAULT_OST_TOML)
            .map_err(|e| format!("{}", crate::i18n::t("err.create_dir", &["opensteamtool.toml", &e.to_string()])))?;
        return Ok(());
    }

    let before = fs::read_to_string(&toml_path)
        .map_err(|e| format!("{}", crate::i18n::t("err.read", &["opensteamtool.toml", &e.to_string()])))?;
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
        .map_err(|e| format!("{}", crate::i18n::t("err.write", &["opensteamtool.toml", &e.to_string()])))
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
            .map_err(|e| format!("{}", crate::i18n::t("err.create_dir", &["opensteamtool.toml", &e.to_string()])))?;
        return Ok("created".to_string());
    }

    let content = fs::read_to_string(&toml_path)
        .map_err(|e| format!("{}", crate::i18n::t("err.read", &["opensteamtool.toml", &e.to_string()])))?;
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
            .map_err(|e| format!("{}", crate::i18n::t("err.write", &["opensteamtool.toml", &e.to_string()])))?;
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
            .map_err(|e| format!("{}", crate::i18n::t("err.write", &["opensteamtool.toml", &e.to_string()])))?;
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
            .map_err(|e| format!("{}", crate::i18n::t("err.write", &["opensteamtool.toml", &e.to_string()])))?;
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
        return Err(crate::i18n::t("err.steam_sh_missing", &[]));
    }

    find_slssteam_file("SLSsteam.so").ok_or_else(|| crate::i18n::t("err.slssteam_missing", &[]))?;
    find_slssteam_file("library-inject.so")
        .ok_or_else(|| crate::i18n::t("err.library_inject_missing", &[]))?;

    // Backup once before the first patch, then rebuild the chain from disk
    // (covers a CloudRedirect that was installed before or after SLSsteam).
    let backup = steam_root.join("steam.sh.bak");
    if !backup.exists() {
        fs::copy(&steam_sh, &backup)
            .map_err(|e| format!("{}", crate::i18n::t("err.create_dir", &["steam.sh.bak", &e.to_string()])))?;
    }
    sync_ld_audit(steam_root)?;

    // Block client self-updates so the LD_AUDIT patch survives.
    fs::write(
        steam_root.join("steam.cfg"),
        "BootStrapperInhibitAll=enable\nBootStrapperForceSelfUpdate=disable\n",
    )
    .map_err(|e| format!("{}", crate::i18n::t("err.create_dir", &["steam.cfg", &e.to_string()])))?;

    // Seed a default config only when none exists — never clobber user data.
    let config_path = slssteam_config_path();
    if !config_path.exists() {
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("{}", crate::i18n::t("err.create_dir", &[&parent.display().to_string(), &e.to_string()])))?;
        }
        fs::write(&config_path, DEFAULT_SLSSTEAM_CONFIG)
            .map_err(|e| format!("{}", crate::i18n::t("err.create_dir", &["config.yaml", &e.to_string()])))?;
    }

    deploy_slssteam_plugins()?;

    Ok(())
}

/// Write the bundled SLSsteam Lua plugins, replacing files whose content
/// differs. Needed for the in-library download flow (`Plugins: yes`).
fn deploy_slssteam_plugins() -> Result<(), String> {
    let plugins_dir = slssteam_config_path()
        .parent()
        .map(|p| p.join("plugins"))
        .ok_or_else(|| "config path has no parent".to_string())?;
    fs::create_dir_all(&plugins_dir).map_err(|e| {
        format!("{}", crate::i18n::t("err.create_dir", &[&plugins_dir.display().to_string(), &e.to_string()]))
    })?;

    for (name, content) in [
        (
            "download.lua",
            include_str!("slssteam_plugins/download.lua"),
        ),
        (
            "spliced-tickets.lua",
            include_str!("slssteam_plugins/spliced-tickets.lua"),
        ),
    ] {
        let path = plugins_dir.join(name);
        let changed = match fs::read_to_string(&path) {
            Ok(existing) => existing != content,
            Err(_) => true,
        };
        if changed {
            fs::write(&path, content).map_err(|e| {
                format!("{}", crate::i18n::t("err.write", &[name, &e.to_string()]))
            })?;
        }
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

    write_sh_force(path, &new_lines.join("\n"))
}

/// `fs::write` for steam.sh that copes with read-only launchers: headcrab
/// replaces steam.sh with its own script and leaves it at mode 555 (no write
/// bit for anyone), which makes a plain write fail with EACCES. The file is
/// user-owned, so owner-write is added for the write and the original mode
/// restored afterwards — keeping the update protection headcrab intended.
#[cfg(unix)]
fn write_sh_force(path: &Path, content: &str) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    let err_write = |e: &std::io::Error| {
        format!("{}", crate::i18n::t("err.write", &["steam.sh", &e.to_string()]))
    };

    let original = fs::metadata(path).map_err(|e| err_write(&e))?.permissions();
    let mode = original.mode() & 0o7777;
    let read_only = mode & 0o200 == 0;
    if read_only {
        fs::set_permissions(path, std::fs::Permissions::from_mode(mode | 0o600))
            .map_err(|e| err_write(&e))?;
    }
    let write_result = fs::write(path, content);
    // Restore even when the write failed — a botched patch must not leave
    // the launcher more writable than it was.
    if read_only {
        let _ = fs::set_permissions(path, original.clone());
    }
    write_result.map_err(|e| err_write(&e))
}

#[cfg(not(unix))]
fn write_sh_force(path: &Path, content: &str) -> Result<(), String> {
    fs::write(path, content)
        .map_err(|e| format!("{}", crate::i18n::t("err.write", &["steam.sh", &e.to_string()])))
}

fn strip_ld_audit(steam_root: &Path) -> Result<(), String> {
    let steam_sh = steam_root.join("steam.sh");
    if !steam_sh.exists() {
        return Ok(());
    }
    let content = fs::read_to_string(&steam_sh)
        .map_err(|e| format!("{}", crate::i18n::t("err.read", &["steam.sh", &e.to_string()])))?;
    if !content.contains("export LD_AUDIT=") {
        return Ok(());
    }
    write_patched_steam_sh(&steam_sh, &content, None)
}

/// Rebuild the `export LD_AUDIT=` line in steam.sh from the components that
/// are currently live on disk: SLSsteam (library-inject + SLSsteam.so) and
/// CloudRedirect's cloud_redirect.so. An empty set strips the line entirely.
/// Called by SlssteamSetup, CloudRedirectSetup and toggle-off so the chain
/// always reflects reality regardless of install/enable order.
fn sync_ld_audit(steam_root: &Path) -> Result<(), String> {
    let steam_sh = steam_root.join("steam.sh");
    if !steam_sh.exists() {
        // Nothing to wire; a missing steam.sh surfaces from the setup hooks.
        return Ok(());
    }

    let mut chain: Vec<String> = Vec::new();
    if let (Some(inject), Some(sls)) = (
        find_slssteam_file("library-inject.so"),
        find_slssteam_file("SLSsteam.so"),
    ) {
        chain.push(inject.display().to_string());
        chain.push(sls.display().to_string());
    }
    if let Some(cr) = find_cloud_redirect_so() {
        chain.push(cr.display().to_string());
    }

    if chain.is_empty() {
        // Only strip when a line is actually present — avoids a pointless
        // rewrite of steam.sh on systems that never had the patch.
        let content = fs::read_to_string(&steam_sh)
            .map_err(|e| format!("{}", crate::i18n::t("err.read", &["steam.sh", &e.to_string()])))?;
        if content.contains("export LD_AUDIT=") {
            return strip_ld_audit(steam_root);
        }
        return Ok(());
    }

    let content = fs::read_to_string(&steam_sh)
        .map_err(|e| format!("{}", crate::i18n::t("err.read", &["steam.sh", &e.to_string()])))?;
    let ld = chain.join(":");
    if content.contains(&format!("export LD_AUDIT={ld}")) {
        return Ok(());
    }
    write_patched_steam_sh(&steam_sh, &content, Some(&ld))
}

/// Live (non-`.bak`) CloudRedirect library in the official Linux location —
/// where headcrab and this panel deploy it (loaded via LD_AUDIT).
fn find_cloud_redirect_so() -> Option<PathBuf> {
    if !cfg!(target_os = "linux") {
        return None;
    }
    let path = dirs::home_dir()?
        .join(".local/share/CloudRedirect")
        .join("cloud_redirect.so");
    path.exists().then_some(path)
}

// ---------------------------------------------------------------------------
// CloudRedirectSetup (Linux)
// ---------------------------------------------------------------------------

/// CloudRedirect on Linux loads through the LD_AUDIT chain that SLSsteam
/// establishes, and only functions while SLSsteam keeps cloud enabled —
/// `DisableCloud: no` is the same gate upstream headcrab greps for.
fn cloud_redirect_setup(steam_root: &Path) -> Result<(), String> {
    if !cfg!(target_os = "linux") {
        return Ok(());
    }
    find_slssteam_file("SLSsteam.so")
        .ok_or_else(|| crate::i18n::t("err.cr_needs_slssteam", &[]))?;
    set_slssteam_disable_cloud(false)?;
    sync_ld_audit(steam_root)
}

/// Flip `DisableCloud:` in the SLSsteam config, preserving everything else.
fn set_slssteam_disable_cloud(cloud_disabled: bool) -> Result<(), String> {
    let path = slssteam_config_path();
    let value = if cloud_disabled { "yes" } else { "no" };
    let content = if path.exists() {
        fs::read_to_string(&path).map_err(|e| {
            format!(
                "{}",
                crate::i18n::t("err.read", &[&path.display().to_string(), &e.to_string()])
            )
        })?
    } else {
        String::new()
    };

    let mut found = false;
    let mut lines: Vec<String> = content
        .lines()
        .map(|line| {
            if line.trim_start().starts_with("DisableCloud:") {
                found = true;
                format!("DisableCloud: {value}")
            } else {
                line.to_string()
            }
        })
        .collect();
    if !found {
        lines.push(format!("DisableCloud: {value}"));
    }
    let mut new_content = lines.join("\n");
    new_content.push('\n');

    if new_content == content {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            format!(
                "{}",
                crate::i18n::t("err.create_dir", &[&parent.display().to_string(), &e.to_string()])
            )
        })?;
    }
    fs::write(&path, &new_content).map_err(|e| {
        format!(
            "{}",
            crate::i18n::t("err.write", &[&path.display().to_string(), &e.to_string()])
        )
    })
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
AdditionalDepots:
DecryptionKeys:
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
Plugins: yes
DisableCloud: yes
DisableUpdates: yes
FakeName: ""
FakeEmail: ""
FakeWalletBalance: 0
LogLevels: 0xff
DumpClientInterfaces: no
ExtendedLogging: no
"#;

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn temp_sh(name: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("lumaforge-postinstall-test-{}-{name}", std::process::id()));
        p
    }

    #[test]
    fn steam_sh_write_works_on_read_only_launcher() {
        // Headcrab leaves its replaced steam.sh at mode 555 — the write must
        // still land and the protective mode must come back untouched.
        let path = temp_sh("readonly.sh");
        let before = "#!/bin/bash\necho hi\n";
        fs::write(&path, before).unwrap();
        fs::set_permissions(&path, std::fs::Permissions::from_mode(0o555)).unwrap();

        write_patched_steam_sh(&path, before, Some("/opt/sls/library-inject.so:/opt/sls/SLSsteam.so"))
            .unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("export LD_AUDIT=/opt/sls/library-inject.so:/opt/sls/SLSsteam.so"));
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o7777;
        assert_eq!(mode, 0o555);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn steam_sh_write_keeps_writable_mode_and_strips_old_line() {
        let path = temp_sh("writable.sh");
        let before = "#!/bin/bash\nexport LD_AUDIT=/old/chain.so\necho hi\n";
        fs::write(&path, before).unwrap();
        fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();

        write_patched_steam_sh(&path, before, Some("/new/chain.so")).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert!(!content.contains("/old/chain.so"));
        assert!(content.contains("export LD_AUDIT=/new/chain.so"));
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o7777;
        assert_eq!(mode, 0o755);
        let _ = fs::remove_file(&path);
    }
}
