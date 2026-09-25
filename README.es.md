<div align="center">

# LumaForge Panel

**Panel de escritorio minimalista para el runtime de Steam de LumaForge — control CDP con un clic, switches de componentes y auto-actualizaciones.**

![Licencia](https://img.shields.io/badge/licencia-GPL--3.0-blue)
![Version](https://img.shields.io/badge/version-0.1.0-purple)
![Plataforma](https://img.shields.io/badge/plataforma-Windows%2010%2F11%20%7C%20Linux-0078d4)
![Rust](https://img.shields.io/badge/Rust-1.77+-orange?logo=rust)
![Tauri](https://img.shields.io/badge/Tauri-v2-FFC131?logo=tauri)
![TypeScript](https://img.shields.io/badge/TypeScript-5.9+-3178c6?logo=typescript)
![React](https://img.shields.io/badge/React-19-61dafb?logo=react)

[English](README.md) · [Espanol](README.es.md)

</div>

---

## Descargo de Responsabilidad

LumaForge Panel es un **proyecto educativo** y una **demostracion tecnica** del desarrollo de aplicaciones de escritorio modernas utilizando Tauri v2, Rust, React y TypeScript. El dashboard **es** la aplicacion: no hay vistas de biblioteca, descargas ni ajustes — solo control de componentes para el runtime de Steam de LumaForge.

**LumaForge Panel no esta afiliado, respaldado ni conectado con Valve Corporation, Steam ni ninguna distribuidora de juegos.** Todas las marcas registradas pertenecen a sus respectivos propietarios.

Este software se proporciona estrictamente con fines educativos y de demostracion. Los usuarios son responsables de garantizar el cumplimiento de todas las leyes aplicables y los terminos de servicio de las plataformas con los que interactuen a traves de este software.

---

## Caracteristicas

| Caracteristica | Descripcion |
|----------------|-------------|
| **Toggle CDP con un clic** | Activa/desactiva el proxy CDP renombrando `wsock32.dll` ↔ `wsock32.dll.bak` en la raiz de Steam, con apagado y reinicio automaticos de Steam cuando hace falta |
| **Switches de componentes** | Activa/desactiva `steam-store-helper`, OpenSteamTool, CloudRedirect y SLS Steam mediante renombres en disco (`.bak`) — el proxy recoge los cambios |
| **Banner de Install/Update** | Descarga con un clic del runtime y herramientas desde GitHub Releases, con progreso de despliegue en vivo |
| **Estado derivado del disco** | Cada estado de componente se lee directamente del disco (`.dll` vs `.bak`) — sin configuracion que se desactualice |
| **Integracion con Steam** | Detecta la instalacion y el estado de Steam; inicia o reinicia Steam desde el panel |
| **System Tray** | Cierra a la bandeja con menu Abrir/Salir |
| **Inicio y apariencia** | Arranque con Windows, arranque minimizado, cerrar a la bandeja, seleccion de tema |
| **Actualizaciones automaticas** | Actualizador firmado (NSIS) con notificaciones dentro de la app |
| **Insignia de actualizaciones** | Consulta GitHub buscando versiones nuevas del runtime/herramientas y las senala en la UI |

---

## Stack Tecnologico

| Capa | Tecnologia |
|------|-----------|
| **Backend** | Rust (Tauri v2) |
| **Frontend** | React 19 + TypeScript 5.9 + Vite 8 |
| **Estilos** | CSS plano con design tokens |
| **HTTP** | reqwest (rustls, blocking) |
| **Instalador** | NSIS (nativo de Tauri v2) |
| **Actualizador** | tauri-plugin-updater (artefactos firmados con minisign) |

---

## Primeros Pasos

### Requisitos

- **Rust** 1.77+ (con `cargo`)
- **Node.js** 20.19+ o 22.12+
- **npm**
- **WebView2** (incluido con Windows 10/11) o **WebKitGTK** (Linux)

### Desarrollo

```bash
git clone https://github.com/eisora08/lumaforge-panel.git
cd lumaforge-panel
npm install
npm run tauri dev
```

### Compilar

```bash
npm run tauri build
```

El instalador estara en `src-tauri/target/release/bundle/nsis/`.

### Releases

Los instaladores precompilados se publican en la pagina de [GitHub Releases](https://github.com/eisora08/lumaforge-panel/releases) y el actualizador integrado los entrega automaticamente a la app.

---

## Estructura del Proyecto

```
lumaforge-panel/
├── src/                          # Frontend (React + TypeScript)
│   ├── components/
│   │   ├── DashboardView.tsx     # Toda la app: hero, switches, banner, updates
│   │   ├── ConfirmModal.tsx      # Confirmaciones de acciones destructivas/instalacion
│   │   └── Toast.tsx             # Notificaciones toast
│   ├── App.tsx                   # Shell: menus, popup de ajustes, checks de actualizador
│   ├── tokens.css / App.css      # Design tokens y estilos
│   └── types.ts                  # Tipos TypeScript compartidos
├── src-tauri/                    # Backend (Rust + Tauri v2)
│   ├── src/
│   │   ├── panel.rs              # Comandos Tauri (estado, toggles, instalaciones)
│   │   ├── tools.rs              # Registro de herramientas + modelo de estado en disco
│   │   ├── github.rs             # Cliente de GitHub Releases (cache 24 h)
│   │   ├── steam.rs              # Deteccion / inicio / reinicio de Steam
│   │   ├── settings.rs           # Preferencias de la app (+ HKCU Run en Windows)
│   │   ├── tray.rs               # Menu de system tray
│   │   └── postinstall.rs        # Hooks de configuracion por herramienta (p. ej. SLS Steam)
│   └── tauri.conf.json           # Configuracion de Tauri (updater, NSIS)
└── .github/workflows/release.yml # Builds de release disparados por tag
```

---

## Proyectos Relacionados

| Repositorio | Licencia | Proposito |
|-----------|---------|---------|
| [lumaforge-cdp-proxy](https://github.com/eisora08/lumaforge-cdp-proxy) | MIT | Runtime CDP/CEF inyectado en Steam |
| [lumaforge-extensions](https://github.com/eisora08/lumaforge-extensions) | MIT | Extension steam-store-helper |

---

## Licencia

Este proyecto esta licenciado bajo la **GNU General Public License v3.0** — ver el archivo [LICENSE](LICENSE) para mas detalles.

Las dependencias de terceros y sus licencias se enumeran en [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
