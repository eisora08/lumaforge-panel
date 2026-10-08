export type ToolId =
  | 'cdp-proxy'
  | 'steam-store-helper'
  | 'opensteamtool'
  | 'cloud_redirect'
  | 'slssteam';

/** Disk-derived state of a deployable component. */
export type ComponentState = 'enabled' | 'disabled' | 'missing';

export interface SteamStatus {
  steamRunning: boolean;
  steamExecutableFound: boolean;
  steamExecutable: string | null;
}

export interface ToolStatus {
  id: string;
  name: string;
  description: string;
  state: ComponentState;
  installed: boolean;
  installedVersion: string | null;
  latestVersion: string | null;
  updateAvailable: boolean;
  releaseAvailable: boolean;
  deployPath: string | null;
}

export interface PanelStatus {
  steamRoot: string | null;
  steam: SteamStatus;
  cdp: ComponentState;
  /** File the CDP toggle renames, per-OS (e.g. `ubuntu12_32/liblumaforge.so`). */
  cdpLoader: string;
  tools: ToolStatus[];
  runtimeInstalled: boolean;
  runtimeUpdateAvailable: boolean;
}

export interface PanelResult {
  ok: boolean;
  message: string;
  /** True when the change only takes effect after Steam restarts. */
  restartRequired: boolean;
}

export interface SteamOpResult {
  ok: boolean;
  status: string;
  message: string;
  steamRunning: boolean;
}

export interface InstallProgress {
  step: 'fetch' | 'download' | 'extract' | 'deploy' | 'setup' | 'done' | 'error';
  message: string;
}

export interface StartupSettings {
  startWithWindows: boolean;
  startMinimized: boolean;
  closeToTray: boolean;
}

export interface AppearanceSettings {
  theme: string;
}

export interface PanelSettings {
  startup: StartupSettings;
  appearance: AppearanceSettings;
  /** UI language: 'en', 'es' or '' (auto-detect on first run). */
  language: string;
}

export interface PartialSettings {
  startup?: Partial<StartupSettings>;
  appearance?: Partial<AppearanceSettings>;
  language?: string;
}

// ---------------------------------------------------------------------------
// Steam themes (skin.json themes managed through the CDP proxy runtime)
// ---------------------------------------------------------------------------

export interface ThemeSlider {
  cssVariable: string;
  min: number;
  max: number;
  step: number;
  unit: string;
}

export interface ThemeCondition {
  key: string;
  description: string;
  tab: string;
  section: string;
  default: unknown;
  /** Dropdown option names; absent for slider conditions. */
  values?: string[] | null;
  slider?: ThemeSlider | null;
  /** Persisted selection from active.json; undefined = theme default. */
  selected?: string | null;
}

export interface ThemeInfo {
  id: string;
  name: string;
  author: string;
  description: string;
  version: string;
  tags: string[];
  previewPath?: string | null;
  conditions: ThemeCondition[];
}

export interface ThemesState {
  /** Directory name of the active theme; empty = theming disabled. */
  active: string;
  themes: ThemeInfo[];
}
