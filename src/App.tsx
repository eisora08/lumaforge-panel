import { useCallback, useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { check, type Update } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import {
  Minus,
  X,
  Settings as SettingsIcon,
  RefreshCw,
} from 'lucide-react';
import { DashboardView } from './components/DashboardView';
import { ConfirmModal } from './components/ConfirmModal';
import { ToastProvider, useToast } from './components/Toast';
import type { PanelSettings, PanelStatus, PartialSettings } from './types';
import './App.css';

const THEMES: { id: string; label: string; swatches: string[] }[] = [
  {
    id: 'midnight-blue',
    label: 'Midnight Blue',
    swatches: ['#070b14', '#0c1d3d', '#60a5fa'],
  },
  {
    id: 'oled-black',
    label: 'OLED Black',
    swatches: ['#000000', '#0a0a0a', '#60a5fa'],
  },
  {
    id: 'steam-gray',
    label: 'Steam Gray',
    swatches: ['#1b2838', '#2a475e', '#66c0ff'],
  },
  {
    id: 'crimson-dark',
    label: 'Crimson Dark',
    swatches: ['#0f0810', '#1f1424', '#f472b6'],
  },
];

type MenuId = 'none' | 'settings' | 'updates';

function applyTheme(theme: string) {
  if (theme === 'midnight-blue') {
    document.documentElement.removeAttribute('data-theme');
  } else {
    document.documentElement.setAttribute('data-theme', theme);
  }
}

function Shell() {
  const toast = useToast();
  const win = getCurrentWindow();

  const [appVersion, setAppVersion] = useState('');
  const [closeModalOpen, setCloseModalOpen] = useState(false);
  const [menu, setMenu] = useState<MenuId>('none');

  const [settings, setSettings] = useState<PanelSettings | null>(null);
  const [status, setStatus] = useState<PanelStatus | null>(null);
  const [updater, setUpdater] = useState<Update | null>(null);

  const [checking, setChecking] = useState(false);
  const [checked, setChecked] = useState(false);
  const [toolBusy, setToolBusy] = useState<string | null>(null);
  const [panelUpdating, setPanelUpdating] = useState(false);
  const [panelProgress, setPanelProgress] = useState<string | null>(null);

  const settingsRef = useRef<HTMLDivElement | null>(null);
  const updatesRef = useRef<HTMLDivElement | null>(null);

  const refreshStatus = useCallback(async () => {
    try {
      setStatus(await invoke<PanelStatus>('panel_status'));
    } catch (error) {
      console.error('[lumaforge-panel] panel_status failed:', error);
    }
  }, []);

  const runUpdaterCheck = useCallback(async () => {
    try {
      const update = await check();
      setUpdater(update);
      return update;
    } catch {
      setUpdater(null);
      return null;
    }
  }, []);

  useEffect(() => {
    invoke<string>('get_app_version')
      .then(setAppVersion)
      .catch(() => setAppVersion('0.1.0'));

    invoke<PanelSettings>('get_settings')
      .then((next) => {
        setSettings(next);
        applyTheme(next.appearance.theme);
      })
      .catch(() => setSettings(null));

    void refreshStatus();
    void runUpdaterCheck();

    const id = window.setInterval(() => void refreshStatus(), 15000);
    return () => window.clearInterval(id);
  }, [refreshStatus, runUpdaterCheck]);

  // Close popovers on outside click / Escape.
  useEffect(() => {
    if (menu === 'none') return;

    const onPointerDown = (event: MouseEvent) => {
      const target = event.target as HTMLElement;
      const inSettings = settingsRef.current?.contains(target);
      const inUpdates = updatesRef.current?.contains(target);
      const onButton = target.closest('[data-menu-toggle]') !== null;
      if (!inSettings && !inUpdates && !onButton) setMenu('none');
    };
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') setMenu('none');
    };

    document.addEventListener('mousedown', onPointerDown);
    document.addEventListener('keydown', onKeyDown);
    return () => {
      document.removeEventListener('mousedown', onPointerDown);
      document.removeEventListener('keydown', onKeyDown);
    };
  }, [menu]);

  const toolUpdates = (status?.tools ?? []).filter(
    (tool) => tool.updateAvailable
  );
  const updateCount = toolUpdates.length + (updater ? 1 : 0);

  const patchSettings = useCallback(
    async (partial: PartialSettings) => {
      try {
        const next = await invoke<PanelSettings>('update_settings', {
          partial,
        });
        setSettings(next);
        applyTheme(next.appearance.theme);
        return next;
      } catch (error) {
        toast.error(
          error instanceof Error ? error.message : String(error)
        );
        return null;
      }
    },
    [toast]
  );

  const runCheckUpdates = useCallback(async () => {
    setChecking(true);
    try {
      const next = await invoke<PanelStatus>('check_updates');
      setStatus(next);
      const freshUpdater = await runUpdaterCheck();
      setChecked(true);
      const count =
        next.tools.filter((tool) => tool.updateAvailable).length +
        (freshUpdater ? 1 : 0);
      if (count > 0) {
        toast.info(`${count} update${count > 1 ? 's' : ''} available.`);
      } else {
        toast.success('Everything is up to date.');
      }
    } catch (error) {
      toast.error(
        error instanceof Error ? error.message : String(error)
      );
    } finally {
      setChecking(false);
    }
  }, [runUpdaterCheck, toast]);

  const runToolUpdate = useCallback(
    async (toolId: string) => {
      setToolBusy(toolId);
      try {
        const result = await invoke<{
          ok: boolean;
          message: string;
          restartRequired: boolean;
        }>('install_tool', { toolId });
        if (result.ok) {
          toast.success(result.message);
          if (result.restartRequired) {
            toast.warning('Restart Steam to finish applying the update.');
          }
        } else {
          toast.error(result.message);
        }
      } catch (error) {
        toast.error(
          error instanceof Error ? error.message : String(error)
        );
      } finally {
        setToolBusy(null);
        await refreshStatus();
        setMenu('none');
      }
    },
    [refreshStatus, toast]
  );

  const runPanelUpdate = useCallback(async () => {
    if (!updater) return;
    setPanelUpdating(true);
    setPanelProgress(null);
    try {
      await updater.downloadAndInstall((event) => {
        if (event.event === 'Progress') {
          setPanelProgress('Downloading...');
        } else if (event.event === 'Finished') {
          setPanelProgress('Installing...');
        } else {
          setPanelProgress('Downloading...');
        }
      });
      await relaunch();
    } catch (error) {
      toast.error(
        error instanceof Error ? error.message : String(error)
      );
      setPanelUpdating(false);
      setPanelProgress(null);
    }
  }, [toast, updater]);

  const handleMinimize = useCallback(() => {
    void win.minimize();
  }, [win]);

  const handleClose = useCallback(() => {
    if (settings?.startup.closeToTray) {
      void win.hide();
      return;
    }
    setCloseModalOpen(true);
  }, [settings, win]);

  return (
    <div className="app-shell">
      <main className="app-main">
        <header className="app-header" data-tauri-drag-region>
          <div className="header-left">
            <span className="header-icon" data-tauri-drag-region>
              <img src="/icon.svg" alt="" aria-hidden="true" />
            </span>
            <span className="app-header-title" data-tauri-drag-region>
              LumaForge
            </span>
            {updateCount > 0 && (
              <button
                type="button"
                className="update-badge"
                data-tauri-drag-region="noDrag"
                data-menu-toggle
                aria-label={`${updateCount} updates available`}
                onClick={() =>
                  setMenu((current) =>
                    current === 'updates' ? 'none' : 'updates'
                  )
                }
              >
                ({updateCount})
              </button>
            )}
          </div>

          <div className="header-spacer" data-tauri-drag-region />

          <div className="header-right" data-tauri-drag-region>
            <button
              type="button"
              className="window-btn window-btn-gear"
              data-tauri-drag-region="noDrag"
              data-menu-toggle
              aria-label="Settings"
              aria-expanded={menu === 'settings'}
              onClick={() =>
                setMenu((current) =>
                  current === 'settings' ? 'none' : 'settings'
                )
              }
            >
              <SettingsIcon size={13} aria-hidden="true" />
            </button>

            <button
              type="button"
              className="window-btn window-btn-minimize"
              data-tauri-drag-region="noDrag"
              onClick={handleMinimize}
              aria-label="Minimize window"
            >
              <Minus size={12} aria-hidden="true" />
            </button>

            <button
              type="button"
              className="window-btn window-btn-close"
              data-tauri-drag-region="noDrag"
              onClick={handleClose}
              aria-label="Close window"
            >
              <X size={12} aria-hidden="true" />
            </button>

            {menu === 'settings' && (
              <div
                className="header-menu"
                ref={settingsRef}
                data-tauri-drag-region="noDrag"
                role="menu"
                aria-label="Settings"
              >
                <div className="menu-section-title">Updates</div>
                <button
                  type="button"
                  className="menu-row menu-row--button"
                  onClick={() => void runCheckUpdates()}
                  disabled={checking}
                >
                  <RefreshCw
                    size={14}
                    aria-hidden="true"
                    className={checking ? 'menu-spin' : undefined}
                  />
                  <span>
                    {checking ? 'Checking for updates...' : 'Check for updates'}
                  </span>
                  <span className="menu-row-status">
                    {checking
                      ? ''
                      : checked
                        ? updateCount > 0
                          ? `${updateCount} available`
                          : 'Up to date'
                        : ''}
                  </span>
                </button>

                <div className="menu-section-title">Startup</div>
                <label className="menu-row" htmlFor="startup-windows">
                  <span>Start with Windows</span>
                  <button
                    id="startup-windows"
                    type="button"
                    role="switch"
                    className="toggle"
                    aria-checked={settings?.startup.startWithWindows ?? false}
                    disabled={settings === null}
                    onClick={() =>
                      void patchSettings({
                        startup: {
                          startWithWindows:
                            !settings?.startup.startWithWindows,
                        },
                      })
                    }
                  >
                    <span className="toggle-track" />
                  </button>
                </label>
                <label className="menu-row" htmlFor="startup-minimized">
                  <span>Start minimized to tray</span>
                  <button
                    id="startup-minimized"
                    type="button"
                    role="switch"
                    className="toggle"
                    aria-checked={settings?.startup.startMinimized ?? false}
                    disabled={settings === null}
                    onClick={() =>
                      void patchSettings({
                        startup: {
                          startMinimized: !settings?.startup.startMinimized,
                        },
                      })
                    }
                  >
                    <span className="toggle-track" />
                  </button>
                </label>
                <label className="menu-row" htmlFor="startup-close-tray">
                  <span>Close button hides to tray</span>
                  <button
                    id="startup-close-tray"
                    type="button"
                    role="switch"
                    className="toggle"
                    aria-checked={settings?.startup.closeToTray ?? false}
                    disabled={settings === null}
                    onClick={() =>
                      void patchSettings({
                        startup: {
                          closeToTray: !settings?.startup.closeToTray,
                        },
                      })
                    }
                  >
                    <span className="toggle-track" />
                  </button>
                </label>

                <div className="menu-section-title">Appearance</div>
                <div className="menu-swatches" role="radiogroup" aria-label="Theme">
                  {THEMES.map((theme) => (
                    <button
                      key={theme.id}
                      type="button"
                      role="radio"
                      aria-checked={settings?.appearance.theme === theme.id}
                      aria-label={theme.label}
                      title={theme.label}
                      className={`menu-swatch ${
                        settings?.appearance.theme === theme.id
                          ? 'menu-swatch--active'
                          : ''
                      }`}
                      onClick={() =>
                        void patchSettings({ appearance: { theme: theme.id } })
                      }
                    >
                      {theme.swatches.map((color) => (
                        <span
                          key={color}
                          style={{ background: color }}
                          aria-hidden="true"
                        />
                      ))}
                    </button>
                  ))}
                </div>

                <div className="menu-footer">
                  LumaForge Panel {appVersion ? `v${appVersion}` : ''}
                </div>
              </div>
            )}

            {menu === 'updates' && (
              <div
                className="header-menu"
                ref={updatesRef}
                data-tauri-drag-region="noDrag"
                role="menu"
                aria-label="Updates"
              >
                <div className="menu-section-title">Updates available</div>

                {updater && (
                  <div className="menu-update-row">
                    <span className="menu-update-copy">
                      <strong>LumaForge Panel</strong>
                      <span>v{updater.version}</span>
                    </span>
                    <button
                      type="button"
                      className="btn btn-primary btn-sm"
                      disabled={panelUpdating}
                      onClick={() => void runPanelUpdate()}
                    >
                      {panelUpdating
                        ? (panelProgress ?? 'WORKING...')
                        : 'UPDATE'}
                    </button>
                  </div>
                )}

                {toolUpdates.map((tool) => (
                  <div className="menu-update-row" key={tool.id}>
                    <span className="menu-update-copy">
                      <strong>{tool.name}</strong>
                      <span>{tool.latestVersion ?? 'new release'}</span>
                    </span>
                    <button
                      type="button"
                      className="btn btn-primary btn-sm"
                      disabled={toolBusy !== null}
                      onClick={() => void runToolUpdate(tool.id)}
                    >
                      {toolBusy === tool.id ? 'WORKING...' : 'UPDATE'}
                    </button>
                  </div>
                ))}
              </div>
            )}
          </div>
        </header>

        <div className="app-view">
          <DashboardView />
        </div>
      </main>

      <ConfirmModal
        open={closeModalOpen}
        title="Close LumaForge Panel"
        description="Minimize to the system tray to keep the panel handy, or close the application completely?"
        confirmLabel="CLOSE APP"
        cancelLabel="MINIMIZE TO TRAY"
        tone="warning"
        autoFocus="cancel"
        onCancel={() => {
          setCloseModalOpen(false);
          void win.hide();
        }}
        onConfirm={() => {
          setCloseModalOpen(false);
          win.destroy();
        }}
      />
    </div>
  );
}

function App() {
  return (
    <ToastProvider>
      <Shell />
    </ToastProvider>
  );
}

export default App;
