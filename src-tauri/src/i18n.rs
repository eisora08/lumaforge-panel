//! Two-language (en/es) message catalog for every user-facing backend
//! string: toast messages, install progress and tray labels.
//!
//! The active language comes from `settings.language` (`""` = auto-detect
//! from the environment on first run). Call [`set_lang`] whenever the
//! setting changes; messages created before that keep the old language.

use std::sync::atomic::{AtomicU8, Ordering};

const LANG_EN: u8 = 0;
const LANG_ES: u8 = 1;

static LANG: AtomicU8 = AtomicU8::new(LANG_EN);

/// Pick the language: explicit `es`/`en`, or environment sniff when empty.
pub fn init_from_settings(language: &str) {
    if language.eq_ignore_ascii_case("es") || language.eq_ignore_ascii_case("en") {
        set_lang(language);
        return;
    }
    let env = std::env::var("LC_ALL")
        .or_else(|_| std::env::var("LANG"))
        .unwrap_or_default();
    set_lang(if env.to_lowercase().starts_with("es") {
        "es"
    } else {
        "en"
    });
}

pub fn set_lang(language: &str) {
    LANG.store(
        if language.eq_ignore_ascii_case("es") {
            LANG_ES
        } else {
            LANG_EN
        },
        Ordering::Relaxed,
    );
}

/// Translate `key`, substituting `{}` placeholders with `args` in order.
/// Unknown keys fall back to the key itself (visible in logs/toasts).
pub fn t(key: &str, args: &[&str]) -> String {
    let template = if LANG.load(Ordering::Relaxed) == LANG_ES {
        es(key)
    } else {
        en(key)
    };
    fill(template, args)
}

/// Replace each `{}` in `template` with the next entry of `args`.
fn fill(template: &str, args: &[&str]) -> String {
    let mut out = String::with_capacity(template.len() + 16);
    let mut rest = template;
    let mut idx = 0;
    while let Some(pos) = rest.find("{}") {
        out.push_str(&rest[..pos]);
        out.push_str(args.get(idx).copied().unwrap_or(""));
        idx += 1;
        rest = &rest[pos + 2..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fills_placeholders_in_order() {
        set_lang("en");
        assert_eq!(t("tool.installed", &["CDP Proxy", "0.4.0"]), "CDP Proxy v0.4.0 installed.");
        set_lang("es");
        assert_eq!(t("tool.installed", &["CDP Proxy", "0.4.0"]), "CDP Proxy v0.4.0 instalado.");
        set_lang("en");
    }

    #[test]
    fn missing_args_leave_no_placeholder() {
        set_lang("en");
        assert_eq!(t("err.steam_root", &[]), "Steam root not found.");
        assert_eq!(fill("{} and {}", &["a"]), "a and ");
        set_lang("en");
    }
}

// ---------------------------------------------------------------------------
// Catalog
// ---------------------------------------------------------------------------

fn en(key: &str) -> &str {
    match key {
        // tools.rs — actions
        "tool.enabled" => "{} enabled.",
        "tool.disabled" => "{} disabled.",
        "tool.installed" => "{} v{} installed.",
        "tool.already_installed" => "{} is already installed.",
        "tool.unavailable" => "{} is not available on this OS.",
        "tool.uninstalled" => "{} uninstalled.",
        "tool.uninstall_failed" => "Uninstall of {} failed: {}",
        "tool.still_present" => "{}: some deployed files are still present.",
        "err.cdp_def_missing" => "CDP proxy definition not found.",
        "err.unknown_tool" => "Unknown tool: {}",
        // tools.rs — install
        "err.no_release" => "No release available for {} yet: {}",
        "err.download" => "Download failed: {}",
        "err.extract" => "Extraction failed: {}",
        "err.deploy" => "Deploy of {} failed: {}",
        "err.release_missing" => "{} not found in release",
        "err.missing_after_deploy" => "{} missing after deploy",
        "err.tool_dir" => "Failed to create tool dir: {}",
        // tools.rs — progress
        "progress.fetch" => "Fetching {} release...",
        "progress.download" => "Downloading {} v{}...",
        "progress.extract" => "Extracting...",
        "progress.copy_plugin" => "Copying plugin files...",
        "progress.deploy_steam" => "Deploying files to the Steam directory...",
        "progress.setup" => "Running post-install setup...",
        "progress.already_present" => "Files already present - deploying...",
        // filesystem errors
        "err.rename" => "Failed to rename {} -> {}: {}",
        "err.steam_root" => "Steam root not found.",
        "err.steam_root_setup" => "Steam root not found - cannot finish setup.",
        "err.deploy_root" => "Deploy root not found.",
        "err.remove" => "Failed to remove {}: {}",
        "err.restore" => "Failed to restore {}: {}",
        "err.create_dir" => "Failed to create {}: {}",
        "err.open" => "Failed to open {}: {}",
        "err.read" => "Failed to read {}: {}",
        "err.write" => "Failed to write {}: {}",
        "err.copy" => "Failed to copy {}: {}",
        "err.serialize" => "Failed to serialize {}: {}",
        // steam.rs
        "err.steam_exec" => "Steam executable not found.",
        "err.steam_launcher" => "Steam launcher not found.",
        "err.steam_start" => "Failed to start Steam: {}",
        "err.start_unsupported" => "Automatic Steam startup is not supported on this platform.",
        "err.steam_exit_timeout" => "Steam did not close within {} seconds. Close Steam manually and try again.",
        "err.steam_shutdown" => "Failed to request Steam shutdown: {}",
        "err.shutdown_unsupported" => "Steam shutdown is not supported on this platform.",
        "steam.already_running" => "Steam is already running.",
        "steam.started" => "Steam was started.",
        "steam.restarted" => "Steam was restarted.",
        // themes.rs
        "theme.invalid_name" => "Invalid theme name",
        "theme.not_installed" => "Theme '{}' is not installed",
        "theme.sync_suffix" => " \u{2014} applies when Steam next launches",
        "theme.activated" => "Theme '{}' activated{}",
        "theme.disabled" => "Theming disabled{}",
        "theme.condition_updated" => "Condition updated{}",
        "theme.condition_reset" => "Condition reset{}",
        "err.cdp_not_running" => "CDP proxy is not running",
        "err.cdp_no_response" => "No response from CDP proxy",
        "err.ipc_unsupported" => "IPC not supported on this platform",
        // panel.rs — command wrappers
        "err.status_task" => "Status task failed: {}",
        "err.update_check" => "Update check failed: {}",
        "err.cdp_toggle" => "CDP toggle failed: {}",
        "err.tool_toggle" => "Tool toggle failed: {}",
        "err.runtime_install" => "Runtime install failed: {}",
        "err.steam_status_task" => "Steam status task failed: {}",
        "err.steam_start_task" => "Steam start task failed: {}",
        "err.steam_restart_task" => "Steam restart task failed: {}",
        "err.install" => "Install failed: {}",
        "err.uninstall" => "Uninstall failed: {}",
        // tray.rs
        "tray.open" => "Open LumaForge Panel",
        "tray.quit" => "Quit",
        _ => key,
    }
}

fn es(key: &str) -> &str {
    match key {
        "tool.enabled" => "{} activado.",
        "tool.disabled" => "{} desactivado.",
        "tool.installed" => "{} v{} instalado.",
        "tool.already_installed" => "{} ya est\u{e1} instalado.",
        "tool.unavailable" => "{} no est\u{e1} disponible en este sistema.",
        "tool.uninstalled" => "{} desinstalado.",
        "tool.uninstall_failed" => "La desinstalaci\u{f3}n de {} fall\u{f3}: {}",
        "tool.still_present" => "{}: algunos archivos desplegados siguen presentes.",
        "err.cdp_def_missing" => "No se encontr\u{f3} la definici\u{f3}n del proxy CDP.",
        "err.unknown_tool" => "Herramienta desconocida: {}",
        "err.no_release" => "A\u{fa}n no hay release disponible para {}: {}",
        "err.download" => "La descarga fall\u{f3}: {}",
        "err.extract" => "La extracci\u{f3}n fall\u{f3}: {}",
        "err.deploy" => "El despliegue de {} fall\u{f3}: {}",
        "err.release_missing" => "{} no se encuentra en el release",
        "err.missing_after_deploy" => "{} ausente tras el despliegue",
        "err.tool_dir" => "Error al crear el directorio de la herramienta: {}",
        "progress.fetch" => "Obteniendo release de {}...",
        "progress.download" => "Descargando {} v{}...",
        "progress.extract" => "Extrayendo...",
        "progress.copy_plugin" => "Copiando archivos del plugin...",
        "progress.deploy_steam" => "Desplegando archivos en el directorio de Steam...",
        "progress.setup" => "Ejecutando la configuraci\u{f3}n post-instalaci\u{f3}n...",
        "progress.already_present" => "Archivos ya presentes: desplegando...",
        "err.rename" => "Error al renombrar {} -> {}: {}",
        "err.steam_root" => "No se encontr\u{f3} la ra\u{ed}z de Steam.",
        "err.steam_root_setup" => "No se encontr\u{f3} la ra\u{ed}z de Steam: no se puede finalizar la configuraci\u{f3}n.",
        "err.deploy_root" => "No se encontr\u{f3} la ra\u{ed}z de despliegue.",
        "err.remove" => "Error al eliminar {}: {}",
        "err.restore" => "Error al restaurar {}: {}",
        "err.create_dir" => "Error al crear {}: {}",
        "err.open" => "Error al abrir {}: {}",
        "err.read" => "Error al leer {}: {}",
        "err.write" => "Error al escribir {}: {}",
        "err.copy" => "Error al copiar {}: {}",
        "err.serialize" => "Error al serializar {}: {}",
        "err.steam_exec" => "No se encontr\u{f3} el ejecutable de Steam.",
        "err.steam_launcher" => "No se encontr\u{f3} el lanzador de Steam.",
        "err.steam_start" => "Error al iniciar Steam: {}",
        "err.start_unsupported" => "El inicio autom\u{e1}tico de Steam no est\u{e1} soportado en este sistema.",
        "err.steam_exit_timeout" => "Steam no se cerr\u{f3} en {} segundos. Cierra Steam manualmente e int\u{e9}ntalo de nuevo.",
        "err.steam_shutdown" => "Error al solicitar el cierre de Steam: {}",
        "err.shutdown_unsupported" => "El cierre de Steam no est\u{e1} soportado en este sistema.",
        "steam.already_running" => "Steam ya est\u{e1} en ejecuci\u{f3}n.",
        "steam.started" => "Steam se inici\u{f3}.",
        "steam.restarted" => "Steam se reinici\u{f3}.",
        "theme.invalid_name" => "Nombre de tema no v\u{e1}lido",
        "theme.not_installed" => "El tema '{}' no est\u{e1} instalado",
        "theme.sync_suffix" => " \u{2014} se aplica cuando Steam se inicie de nuevo",
        "theme.activated" => "Tema '{}' activado{}",
        "theme.disabled" => "Temas desactivados{}",
        "theme.condition_updated" => "Condici\u{f3}n actualizada{}",
        "theme.condition_reset" => "Condici\u{f3}n restablecida{}",
        "err.cdp_not_running" => "El proxy CDP no est\u{e1} en ejecuci\u{f3}n",
        "err.cdp_no_response" => "Sin respuesta del proxy CDP",
        "err.ipc_unsupported" => "IPC no soportado en este sistema",
        "err.status_task" => "La tarea de estado fall\u{f3}: {}",
        "err.update_check" => "La comprobaci\u{f3}n de actualizaciones fall\u{f3}: {}",
        "err.cdp_toggle" => "El conmutador CDP fall\u{f3}: {}",
        "err.tool_toggle" => "El conmutador de la herramienta fall\u{f3}: {}",
        "err.runtime_install" => "La instalaci\u{f3}n del runtime fall\u{f3}: {}",
        "err.steam_status_task" => "La tarea de estado de Steam fall\u{f3}: {}",
        "err.steam_start_task" => "La tarea de inicio de Steam fall\u{f3}: {}",
        "err.steam_restart_task" => "La tarea de reinicio de Steam fall\u{f3}: {}",
        "err.install" => "La instalaci\u{f3}n fall\u{f3}: {}",
        "err.uninstall" => "La desinstalaci\u{f3}n fall\u{f3}: {}",
        "tray.open" => "Abrir LumaForge Panel",
        "tray.quit" => "Salir",
        _ => key,
    }
}
