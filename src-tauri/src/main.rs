// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // NVIDIA + Wayland: WebKitGTK's DMABUF renderer crashes the web process
    // ("WebKitWebProcess has encountered a fatal error and was closed").
    // Must run before GTK initializes.
    #[cfg(target_os = "linux")]
    webkit2gtk_nvidia_quirk::apply_workaround_with_options(
        webkit2gtk_nvidia_quirk::ApplyWorkaroundOptions::default(),
    );

    lumaforge_panel_lib::run();
}
