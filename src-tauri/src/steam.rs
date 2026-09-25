use serde::Serialize;
use std::path::PathBuf;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use crate::paths;

const STEAM_EXIT_TIMEOUT_SECS: u64 = 30;
const PROCESS_POLL_INTERVAL_MS: u64 = 500;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SteamStatus {
    pub steam_running: bool,
    pub steam_executable_found: bool,
    pub steam_executable: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SteamOpResult {
    pub ok: bool,
    pub status: String,
    pub message: String,
    pub steam_running: bool,
}

pub fn resolve_steam_executable() -> Option<PathBuf> {
    let steam_root = paths::detect_steam_root()?;
    let executable = steam_root.join("steam.exe");
    if executable.is_file() {
        Some(executable)
    } else {
        None
    }
}

#[cfg(target_os = "windows")]
pub fn is_steam_running() -> bool {
    use std::os::windows::process::CommandExt;

    let output = Command::new("tasklist")
        .args(["/FI", "IMAGENAME eq steam.exe", "/FO", "CSV", "/NH"])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    let output = match output {
        Ok(output) => output,
        Err(error) => {
            eprintln!("[STEAM] Failed to query Steam process: {error}");
            return false;
        }
    };

    if !output.status.success() {
        return false;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .any(|line| line.trim_start().to_ascii_lowercase().starts_with("\"steam.exe\""))
}

#[cfg(not(target_os = "windows"))]
pub fn is_steam_running() -> bool {
    false
}

pub fn current_status() -> SteamStatus {
    let executable = resolve_steam_executable();
    SteamStatus {
        steam_running: is_steam_running(),
        steam_executable_found: executable.is_some(),
        steam_executable: executable.map(|p| p.to_string_lossy().to_string()),
    }
}

fn op_result(ok: bool, status: impl Into<String>, message: impl Into<String>) -> SteamOpResult {
    let status_now = current_status();
    SteamOpResult {
        ok,
        status: status.into(),
        message: message.into(),
        steam_running: status_now.steam_running,
    }
}

#[cfg(target_os = "windows")]
fn launch_steam_process() -> Result<(), String> {
    use std::os::windows::process::CommandExt;

    let executable =
        resolve_steam_executable().ok_or_else(|| "Steam executable not found.".to_string())?;

    eprintln!("[STEAM] Starting Steam: {}", executable.display());

    Command::new(&executable)
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|error| format!("Failed to start Steam: {error}"))?;

    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn launch_steam_process() -> Result<(), String> {
    Err("Automatic Steam startup is only supported on Windows.".to_string())
}

pub fn wait_for_steam_exit(timeout: Duration) -> Result<(), String> {
    let started_at = Instant::now();

    while started_at.elapsed() < timeout {
        if !is_steam_running() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(PROCESS_POLL_INTERVAL_MS));
    }

    Err(format!(
        "Steam did not close within {} seconds. Close Steam manually and try again.",
        timeout.as_secs()
    ))
}

#[cfg(target_os = "windows")]
fn request_normal_steam_exit() -> Result<(), String> {
    use std::os::windows::process::CommandExt;

    if !is_steam_running() {
        return Ok(());
    }

    eprintln!("[STEAM] Requesting normal shutdown");

    if let Some(executable) = resolve_steam_executable() {
        match Command::new(&executable)
            .arg("-shutdown")
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
        {
            Ok(_) => return Ok(()),
            Err(error) => eprintln!("[STEAM] steam.exe -shutdown failed: {error}"),
        }
    }

    eprintln!("[STEAM] Falling back to steam://exit");
    Command::new("cmd")
        .args(["/C", "start", "", "steam://exit"])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|error| format!("Failed to request Steam shutdown: {error}"))?;

    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn request_normal_steam_exit() -> Result<(), String> {
    Err("Steam shutdown is only supported on Windows.".to_string())
}

/// Stop Steam gracefully when it is running.
/// Returns `true` when Steam had to be stopped (caller must offer a restart).
pub fn ensure_steam_stopped() -> Result<bool, String> {
    if !is_steam_running() {
        return Ok(false);
    }

    request_normal_steam_exit()?;
    wait_for_steam_exit(Duration::from_secs(STEAM_EXIT_TIMEOUT_SECS))?;
    Ok(true)
}

fn op_result_running(
    ok: bool,
    status: impl Into<String>,
    message: impl Into<String>,
) -> SteamOpResult {
    op_result(ok, status, message)
}

pub fn start_steam() -> SteamOpResult {
    if is_steam_running() {
        return op_result_running(true, "already_running", "Steam is already running.");
    }

    if resolve_steam_executable().is_none() {
        return op_result_running(false, "steam_not_found", "Steam executable not found.");
    }

    match launch_steam_process() {
        Ok(()) => op_result_running(true, "started", "Steam was started."),
        Err(error) => op_result_running(false, "start_failed", error),
    }
}

pub fn restart_steam() -> SteamOpResult {
    if resolve_steam_executable().is_none() {
        return op_result_running(false, "steam_not_found", "Steam executable not found.");
    }

    if is_steam_running() {
        if let Err(error) = request_normal_steam_exit() {
            return op_result_running(false, "exit_request_failed", error);
        }
        if let Err(error) = wait_for_steam_exit(Duration::from_secs(STEAM_EXIT_TIMEOUT_SECS)) {
            return op_result_running(false, "steam_exit_timeout", error);
        }
    }

    match launch_steam_process() {
        Ok(()) => op_result_running(true, "restarted", "Steam was restarted."),
        Err(error) => op_result_running(false, "start_failed", error),
    }
}
