# Third-Party Notices

This file contains the licenses and attributions for all third-party software, libraries, and services used by LumaForge Panel.

---

## Rust Crates (16 direct)

| Crate | License | Purpose |
|-------|---------|---------|
| `tauri` (2.x) | MIT | Application framework (tray-icon feature) |
| `tauri-build` | MIT | Build tooling |
| `tauri-plugin-updater` | MIT | Signed auto-updates |
| `tauri-plugin-process` | MIT | Process management (restart after update) |
| `tray-icon` | MIT | System tray icon |
| `muda` | MIT | Native menu library (tray menu) |
| `serde` / `serde_json` | MIT | JSON serialization |
| `reqwest` (rustls) | MIT | HTTPS client (GitHub Releases API) |
| `zip` | MIT | ZIP archive extraction |
| `sevenz-rust` | MIT | 7z archive extraction |
| `dirs` | MIT | Standard directory paths |
| `anyhow` | MIT | Error handling |
| `chrono` | MIT | Date/time handling |
| `winreg` (Windows only) | MIT | HKCU Run key (start with Windows) |
| `minisign-verify` | MIT | Updater signature verification |
| `lucide-react` (npm) | MIT | Icon library |

> **Note:** All crates listed above are MIT-licensed unless otherwise noted. The full dependency tree (~540 crates) consists of transitive dependencies of the listed crates; run `cargo license` for a complete audit.

---

## Runtime npm Packages (6)

| Package | License | Purpose |
|---------|---------|---------|
| `@tauri-apps/api` | MIT | Tauri JavaScript API |
| `@tauri-apps/plugin-process` | MIT | Process management API |
| `@tauri-apps/plugin-updater` | MIT | Auto-update API |
| `react` (19.x) | MIT | UI framework |
| `react-dom` (19.x) | MIT | React DOM renderer |
| `lucide-react` | MIT | Icon library |

---

## Dev npm Packages (7)

| Package | License | Purpose |
|---------|---------|---------|
| `@tauri-apps/cli` | MIT | Tauri CLI tooling |
| `@types/react` | MIT | React type definitions |
| `@types/react-dom` | MIT | React DOM types |
| `@vitejs/plugin-react` | MIT | Vite React plugin |
| `typescript` (5.9) | Apache-2.0 | TypeScript compiler |
| `vite` (8.x) | MIT | Build tool |

---

## External Tools (Runtime Downloads)

These tools are **not bundled** with LumaForge Panel. They are downloaded from their official GitHub Releases when the user explicitly clicks Install or enables a component in the dashboard.

### LumaForge CDP Proxy

- **Repository:** [eisora08/lumaforge-cdp-proxy](https://github.com/eisora08/lumaforge-cdp-proxy)
- **License:** MIT
- **Purpose:** `wsock32.dll` bootstrap loader + LumaForge DLLs injected into the Steam client (CDP/CEF integration)

### steam-store-helper

- **Repository:** [eisora08/lumaforge-extensions](https://github.com/eisora08/lumaforge-extensions)
- **License:** MIT
- **Purpose:** Steam store detection extension (manifest download, library folders)

### OpenSteamTool

- **Repository:** [eisora08/OpenSteamTool](https://github.com/eisora08/OpenSteamTool)
- **License:** GPL-3.0
- **Purpose:** Open-source Steam unlocker with Lua scripting

### CloudRedirect

- **Repository:** [Selectively11/CloudRedirect](https://github.com/Selectively11/CloudRedirect)
- **License:** MIT
- **Purpose:** Steam Cloud save redirection to Google Drive, OneDrive, S3, R2 or local folders

### SLS Steam

- **Repository:** [AceSLS/SLSsteam](https://github.com/AceSLS/SLSsteam)
- **License:** AGPL-3.0
- **Purpose:** `LD_AUDIT`-based Steam unlocker for Linux

---

## External APIs

These services are accessed via their public APIs. No API keys are bundled or required.

| Service | API Documentation | Purpose |
|---------|-------------------|---------|
| **GitHub Releases** | [REST API](https://docs.github.com/en/rest/releases) | Fetching tool/runtime releases and assets |

---

## Additional Attributions

- **Dashboard design** derived from [LumaForge](https://github.com/eisora08/lumaforge) / luma-lite and inspired by modern launcher interfaces (Steam Desktop, Discord).
- The panel reuses code from **luma-lite** (GPL-3.0) for Steam detection, GitHub release fetching, and runtime management.

---

## License Compliance

LumaForge Panel is licensed under GPL-3.0 (see [LICENSE](LICENSE)). Third-party tools downloaded at runtime are distributed unmodified from their upstream repositories under their own licenses (MIT, GPL-3.0, AGPL-3.0 as listed above); the panel never modifies their sources.

For questions about licensing, contact the project maintainers.
