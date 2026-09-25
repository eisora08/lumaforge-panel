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
}

export interface PartialSettings {
  startup?: Partial<StartupSettings>;
  appearance?: Partial<AppearanceSettings>;
}
