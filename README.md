<div align="center">

# LumaForge Panel

**Minimal desktop dashboard for the LumaForge Steam runtime — one-click CDP control, component switches, and auto-updates.**

![License](https://img.shields.io/badge/license-GPL--3.0-blue)
![Version](https://img.shields.io/badge/version-0.1.1-purple)
![Platform](https://img.shields.io/badge/platform-Windows%2010%2F11%20%7C%20Linux-0078d4)
![Rust](https://img.shields.io/badge/Rust-1.77+-orange?logo=rust)
![Tauri](https://img.shields.io/badge/Tauri-v2-FFC131?logo=tauri)
![TypeScript](https://img.shields.io/badge/TypeScript-5.9+-3178c6?logo=typescript)
![React](https://img.shields.io/badge/React-19-61dafb?logo=react)

[English](README.md) · [Espanol](README.es.md)

</div>

---

## Disclaimer

LumaForge Panel is an **educational project** and a **technical demonstration** of modern desktop application development using Tauri v2, Rust, React, and TypeScript. The dashboard **is** the application: there are no library, downloads, or settings views — just component control for the LumaForge Steam runtime.

**LumaForge Panel is not affiliated with, endorsed by, or connected to Valve Corporation, Steam, or any game publisher.** All trademarks belong to their respective owners.

This software is provided strictly for educational and demonstration purposes. Users are responsible for ensuring compliance with all applicable laws and terms of service for the platforms they interact with through this software.

---

## Features

| Feature | Description |
|---------|-------------|
| **One-Click CDP Toggle** | Enable/disable the CDP proxy by renaming `wsock32.dll` ↔ `wsock32.dll.bak` in the Steam root, with automatic Steam shutdown and restart when needed |
| **Component Switches** | Toggle `steam-store-helper`, OpenSteamTool, CloudRedirect and SLS Steam on/off via disk renames (`.bak`) — the proxy picks up the changes |
| **Install/Update Banner** | One-click download of the runtime and tools from GitHub Releases, with live deploy progress |
| **Disk-Derived State** | Every component status is read straight from disk (`.dll` vs `.bak`) — no configuration to go stale |
| **Steam Integration** | Detects the Steam install and running state; start or restart Steam from the dashboard |
| **System Tray** | Close-to-tray with Open/Quit menu |
| **Startup & Appearance** | Start with Windows, start minimized, close to tray, theme selection |
| **Auto-Updates** | Signed updater (NSIS) with in-app update notifications |
| **Live Update Badge** | Checks GitHub for newer runtime/tool versions and flags them in the UI |

---

## Tech Stack

| Layer | Technology |
|-------|-----------|
| **Backend** | Rust (Tauri v2) |
| **Frontend** | React 19 + TypeScript 5.9 + Vite 8 |
| **Styling** | Plain CSS with design tokens |
| **HTTP** | reqwest (rustls, blocking) |
| **Installer** | NSIS (Tauri v2 native) |
| **Updater** | tauri-plugin-updater (minisign-signed artifacts) |

---

## Getting Started

### Requirements

- **Rust** 1.77+ (with `cargo`)
- **Node.js** 20.19+ or 22.12+
- **npm**
- **WebView2** (ships with Windows 10/11) or **WebKitGTK** (Linux)

### Development

```bash
git clone https://github.com/eisora08/lumaforge-panel.git
cd lumaforge-panel
npm install
npm run tauri dev
```

### Build

```bash
npm run tauri build
```

The installer will be in `src-tauri/target/release/bundle/nsis/`.

### Releases

Prebuilt installers are published on the [GitHub Releases](https://github.com/eisora08/lumaforge-panel/releases) page and delivered to the app by the built-in updater.

---

## Project Structure

```
lumaforge-panel/
├── src/                          # Frontend (React + TypeScript)
│   ├── components/
│   │   ├── DashboardView.tsx     # The whole app: hero, switches, banner, updates
│   │   ├── ConfirmModal.tsx      # Destructive-action / install confirmations
│   │   └── Toast.tsx             # Toast notifications
│   ├── App.tsx                   # Shell: menus, settings popup, updater checks
│   ├── tokens.css / App.css      # Design tokens & styles
│   └── types.ts                  # Shared TypeScript types
├── src-tauri/                    # Backend (Rust + Tauri v2)
│   ├── src/
│   │   ├── panel.rs              # Tauri commands (status, toggles, installs)
│   │   ├── tools.rs              # Tool registry + disk state model
│   │   ├── github.rs             # GitHub Releases client (24 h cache)
│   │   ├── steam.rs              # Steam detection / start / restart
│   │   ├── settings.rs           # App preferences (+ HKCU autostart)
│   │   ├── tray.rs               # System tray menu
│   │   └── postinstall.rs        # Per-tool setup hooks (e.g. SLS Steam)
│   └── tauri.conf.json           # Tauri configuration (updater, NSIS)
└── .github/workflows/release.yml # Tag-triggered release builds
```

---

## Related Projects

| Repository | License | Purpose |
|-----------|---------|---------|
| [lumaforge-cdp-proxy](https://github.com/eisora08/lumaforge-cdp-proxy) | MIT | CDP/CEF runtime injected into Steam |
| [lumaforge-extensions](https://github.com/eisora08/lumaforge-extensions) | MIT | steam-store-helper extension |

---

## License

This project is licensed under the **GNU General Public License v3.0** — see the [LICENSE](LICENSE) file for details.

Third-party dependencies and their licenses are listed in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
