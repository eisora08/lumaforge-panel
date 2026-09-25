import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { ConfirmModal } from './ConfirmModal';
import { useToast } from './Toast';
import type {
  ComponentState,
  InstallProgress,
  PanelResult,
  PanelStatus,
  SteamOpResult,
  ToolStatus,
} from '../types';
import {
  Power,
  Blocks,
  Puzzle,
  Terminal,
  Cloud,
  Shield,
  RefreshCw,
  Download,
} from 'lucide-react';

type BusyKind = 'cdp' | 'tool' | 'steam' | null;

type ModalKind =
  | { kind: 'install-tool'; toolId: string; toolName: string }
  | { kind: 'install-runtime' }
  | null;

const TOOL_ICONS: Record<string, ReactNode> = {
  'steam-store-helper': <Puzzle size={16} aria-hidden="true" />,
  opensteamtool: <Terminal size={16} aria-hidden="true" />,
  cloud_redirect: <Cloud size={16} aria-hidden="true" />,
  'cdp-proxy': <Blocks size={16} aria-hidden="true" />,
  slssteam: <Shield size={16} aria-hidden="true" />,
};

function stateBadge(state: ComponentState): { label: string; cls: string } {
  switch (state) {
    case 'enabled':
      return { label: 'Active', cls: 'status-ok' };
    case 'disabled':
      return { label: 'Backed up', cls: 'status-label' };
    case 'missing':
      return { label: 'Not installed', cls: 'status-warn' };
  }
}

export function DashboardView() {
  const toast = useToast();

  const [status, setStatus] = useState<PanelStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState<BusyKind>(null);
  const [modal, setModal] = useState<ModalKind>(null);
  const [progress, setProgress] = useState<InstallProgress | null>(null);
  const [modalResult, setModalResult] = useState<
    'success' | 'error' | null
  >(null);
  const [restartOpen, setRestartOpen] = useState(false);

  const listenerRef = useRef<(() => void) | null>(null);
  const pendingRestartRef = useRef(false);

  const refresh = useCallback(async () => {
    try {
      const next = await invoke<PanelStatus>('panel_status');
      setStatus(next);
    } catch (error) {
      console.error('[lumaforge-panel] panel_status failed:', error);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
    const id = window.setInterval(() => {
      void refresh();
    }, 15000);
    return () => window.clearInterval(id);
  }, [refresh]);

  const closeModal = useCallback(() => {
    if (listenerRef.current) {
      listenerRef.current();
      listenerRef.current = null;
    }
    setModal(null);
    setProgress(null);
    setModalResult(null);
  }, []);

  const runWithProgress = useCallback(
    async (action: () => Promise<PanelResult>) => {
      setProgress(null);
      setModalResult(null);

      const unlisten = await listen<InstallProgress>(
        'panel://progress',
        (event) => {
          setProgress(event.payload);
        }
      );
      listenerRef.current = unlisten;

      try {
        const result = await action();
        if (!result.ok) {
          setModalResult('error');
          setProgress({ step: 'error', message: result.message });
          toast.error(result.message);
          return;
        }
        setModalResult('success');
        setProgress({ step: 'done', message: result.message });
        toast.success(result.message);
        pendingRestartRef.current = result.restartRequired;
      } catch (error) {
        const message =
          error instanceof Error ? error.message : String(error);
        setModalResult('error');
        setProgress({ step: 'error', message });
        toast.error(message);
      } finally {
        unlisten();
        listenerRef.current = null;
        await refresh();
      }
    },
    [refresh, toast]
  );

  const handleCdpToggle = useCallback(async () => {
    if (!status || busy) return;
    const desired = status.cdp !== 'enabled';

    if (status.cdp === 'missing') {
      setModal({ kind: 'install-runtime' });
      return;
    }

    setBusy('cdp');
    try {
      const result = await invoke<PanelResult>('cdp_set_enabled', {
        enabled: desired,
      });
      if (result.ok) {
        toast.success(result.message);
        pendingRestartRef.current = result.restartRequired;
      } else {
        toast.error(result.message);
      }
    } catch (error) {
      toast.error(
        error instanceof Error ? error.message : String(error)
      );
    } finally {
      setBusy(null);
      await refresh();
      if (pendingRestartRef.current) {
        pendingRestartRef.current = false;
        setRestartOpen(true);
      }
    }
  }, [status, busy, refresh, toast]);

  const handleToolToggle = useCallback(
    async (tool: ToolStatus) => {
      if (busy) return;
      const desired = tool.state !== 'enabled';

      if (desired && !tool.installed) {
        setModal({
          kind: 'install-tool',
          toolId: tool.id,
          toolName: tool.name,
        });
        return;
      }

      setBusy('tool');
      try {
        const result = await invoke<PanelResult>('set_tool_enabled', {
          toolId: tool.id,
          enabled: desired,
        });
        if (result.ok) {
          toast.success(result.message);
          pendingRestartRef.current = result.restartRequired;
        } else {
          toast.error(result.message);
        }
      } catch (error) {
        toast.error(
          error instanceof Error ? error.message : String(error)
        );
      } finally {
        setBusy(null);
        await refresh();
        if (pendingRestartRef.current) {
          pendingRestartRef.current = false;
          setRestartOpen(true);
        }
      }
    },
    [busy, refresh, toast]
  );

  const openInstallModal = useCallback(
    (kind: ModalKind) => {
      setModal(kind);
      void runWithProgress(async () => {
        if (kind?.kind === 'install-tool') {
          return invoke<PanelResult>('install_tool', {
            toolId: kind.toolId,
          });
        }
        return invoke<PanelResult>('install_runtime');
      });
    },
    [runWithProgress]
  );

  const startSteam = useCallback(async () => {
    if (busy) return;
    setBusy('steam');
    try {
      const result = await invoke<SteamOpResult>('start_steam');
      if (result.ok) toast.success(result.message);
      else toast.error(result.message);
    } catch (error) {
      toast.error(
        error instanceof Error ? error.message : String(error)
      );
    } finally {
      setBusy(null);
      await refresh();
    }
  }, [busy, refresh, toast]);

  const restartSteam = useCallback(async () => {
    setBusy('steam');
    try {
      const result = await invoke<SteamOpResult>('restart_steam');
      if (result.ok) {
        toast.success(result.message);
        setRestartOpen(false);
      } else {
        toast.error(result.message);
      }
    } catch (error) {
      toast.error(
        error instanceof Error ? error.message : String(error)
      );
    } finally {
      setBusy(null);
      await refresh();
    }
  }, [refresh, toast]);

  const cdpState: ComponentState = status?.cdp ?? 'missing';
  const cdpBusy = busy === 'cdp';

  const cdpStatusCopy = useMemo(() => {
    if (cdpBusy) {
      return {
        eyebrow: 'APPLYING CHANGES',
        title: 'Updating CDP injection…',
        description: 'Steam may restart to apply the new state.',
        buttonLabel: 'WAIT',
        description2: 'Operation in progress',
      };
    }
    if (loading && !status) {
      return {
        eyebrow: 'CHECKING SYSTEM',
        title: 'Verifying system state',
        description: 'Reading components from disk.',
        buttonLabel: 'CHECKING',
        description2: 'Reading system state',
      };
    }
    if (cdpState === 'missing') {
      return {
        eyebrow: 'RUNTIME NOT INSTALLED',
        title: 'Install the LumaForge runtime',
        description:
          'The CDP injection DLLs are not present in the Steam directory yet.',
        buttonLabel: 'INSTALL',
        description2: 'Download and install the runtime',
      };
    }
    if (cdpState === 'enabled') {
      return {
        eyebrow: 'CDP INJECTION ACTIVE',
        title: 'LumaForge is running',
        description:
          'wsock32.dll is loaded by Steam and extensions are being injected.',
        buttonLabel: 'ENABLED',
        description2: 'Disable CDP injection',
      };
    }
    return {
      eyebrow: 'CDP INJECTION OFF',
      title: 'LumaForge is paused',
      description:
        'The loader is backed up as wsock32.dll.bak and Steam loads vanilla.',
      buttonLabel: 'DISABLED',
      description2: 'Enable CDP injection',
    };
  }, [cdpBusy, cdpState, loading, status]);

  const heroState = cdpBusy
    ? 'busy'
    : loading && !status
      ? 'checking'
      : cdpState === 'missing'
        ? 'missing'
        : cdpState === 'enabled'
          ? 'enabled'
          : 'disabled';

  const enabledTools = useMemo(
    () => status?.tools.filter((t) => t.state === 'enabled').length ?? 0,
    [status]
  );

  const runtimeNeeded = useMemo(() => {
    if (!status) return false;
    if (status.runtimeUpdateAvailable) return true;
    return status.cdp === 'missing';
  }, [status]);

  const steamStatusText = useMemo(() => {
    if (loading && !status) return 'Checking';
    if (status?.steam.steamRunning) return 'Running';
    return 'Stopped';
  }, [status, loading]);

  const openSteamAction = useMemo(() => {
    if (!status) return null;
    if (status.steam.steamRunning) return null;
    if (status.steam.steamExecutableFound) return 'Start Steam';
    return null;
  }, [status]);

  return (
    <>
      <div className="view-content panel-dashboard">
        <main className="panel-column">
          {runtimeNeeded && status && (
            <section
              className={`panel-banner ${
                status.runtimeUpdateAvailable
                  ? 'panel-banner--update'
                  : 'panel-banner--install'
              }`}
            >
              <span className="panel-banner-icon">
                {status.runtimeUpdateAvailable ? (
                  <RefreshCw size={16} aria-hidden="true" />
                ) : (
                  <Download size={16} aria-hidden="true" />
                )}
              </span>

              <span className="panel-banner-copy">
                <strong className="panel-banner-title">
                  {status.runtimeUpdateAvailable
                    ? 'Runtime update available'
                    : 'LumaForge runtime required'}
                </strong>
                <span className="panel-banner-desc">
                  {status.runtimeUpdateAvailable
                    ? 'A newer release of the CDP proxy is published on GitHub.'
                    : 'Installs the CDP proxy DLLs into Steam and the steam-store-helper plugin.'}
                </span>
              </span>

              <button
                type="button"
                className="btn btn-primary btn-sm panel-banner-btn"
                onClick={() => openInstallModal({ kind: 'install-runtime' })}
                disabled={busy !== null}
              >
                {status.runtimeUpdateAvailable ? 'UPDATE' : 'INSTALL'}
              </button>
            </section>
          )}

          <section
            className={`panel-hero panel-hero--${heroState}`}
            aria-label="CDP injection"
          >
            <div className="panel-hero-copy">
              <div className={`dashboard-state dashboard-state--${heroState}`}>
                <span className="dashboard-state-dot" aria-hidden="true" />
                <span>{cdpStatusCopy.eyebrow}</span>
              </div>

              <h2 className="panel-hero-title">{cdpStatusCopy.title}</h2>

              <p className="panel-hero-desc">{cdpStatusCopy.description}</p>

              <div className="panel-hero-facts">
                <span className="panel-fact">
                  Steam: <strong>{steamStatusText}</strong>
                  {openSteamAction && (
                    <button
                      type="button"
                      className="panel-fact-link"
                      onClick={() => {
                        void startSteam();
                      }}
                      disabled={busy !== null}
                    >
                      {openSteamAction}
                    </button>
                  )}
                </span>
                <span className="panel-fact">
                  Components: <strong>{stateBadge(cdpState).label}</strong>
                </span>
                <span className="panel-fact">
                  Components on:{' '}
                  <strong>
                    {enabledTools} of {status?.tools.length ?? 0}
                  </strong>
                </span>
              </div>
            </div>

            <div className="panel-power-area">
              <button
                type="button"
                className={`panel-power panel-power--${heroState}`}
                onClick={() => {
                  void handleCdpToggle();
                }}
                disabled={cdpBusy || (loading && !status)}
                aria-pressed={cdpState === 'enabled'}
                aria-busy={cdpBusy}
                aria-label={
                  cdpState === 'enabled'
                    ? 'Disable CDP injection'
                    : 'Enable CDP injection'
                }
                title={cdpStatusCopy.description2}
              >
                <Power size={26} aria-hidden="true" />
                <span className="panel-power-label">
                  {cdpStatusCopy.buttonLabel}
                </span>
              </button>

              <span className="panel-power-desc">
                {cdpStatusCopy.description2}
              </span>
            </div>
          </section>

          <section className="panel-components" aria-label="Components">
            {status?.tools.map((tool) => {
              const badge = stateBadge(tool.state);
              const toggleBusy = busy === 'tool';

              return (
                <div className="panel-component-row" key={tool.id}>
                  <span className="panel-component-icon">
                    {TOOL_ICONS[tool.id] ?? <Blocks size={16} />}
                  </span>

                  <strong className="panel-component-name">
                    {tool.name}
                  </strong>

                  <span className={`status-tag ${badge.cls}`}>
                    {badge.label}
                  </span>

                  {tool.updateAvailable ? (
                    <button
                      type="button"
                      className="btn btn-secondary btn-sm panel-component-update"
                      disabled={toggleBusy}
                      onClick={() => {
                        openInstallModal({
                          kind: 'install-tool',
                          toolId: tool.id,
                          toolName: tool.name,
                        });
                      }}
                    >
                      UPDATE
                    </button>
                  ) : (
                    <span className="panel-component-version">
                      {tool.installed && tool.installedVersion
                        ? `v${tool.installedVersion}`
                        : ''}
                    </span>
                  )}

                  <button
                    type="button"
                    className="toggle panel-component-toggle"
                    role="switch"
                    aria-checked={tool.state === 'enabled'}
                    aria-label={`Toggle ${tool.name}`}
                    aria-busy={toggleBusy}
                    disabled={toggleBusy || loading}
                    onClick={() => {
                      void handleToolToggle(tool);
                    }}
                  >
                    <span className="toggle-track" />
                  </button>
                </div>
              );
            })}
          </section>

          <div className="panel-footer">
            <span className="panel-footer-path">
              {status?.steamRoot
                ? `Steam · ${status.steamRoot}`
                : 'Steam root not detected'}
            </span>

            <button
              type="button"
              className="btn btn-secondary btn-sm"
              onClick={() => {
                void refresh();
              }}
              disabled={loading || busy !== null}
            >
              Refresh
            </button>
          </div>
        </main>
      </div>

      <ConfirmModal
        open={restartOpen}
        title="Restart Steam to apply changes?"
        description="Steam must restart so it can reload the patched DLLs and pick up the new component state."
        warning="Make sure no game, installation, or download is currently active before restarting Steam."
        confirmLabel="RESTART STEAM"
        cancelLabel="LATER"
        busyLabel="RESTARTING..."
        tone="warning"
        busy={busy === 'steam'}
        autoFocus="cancel"
        closeOnBackdrop
        closeOnEscape
        onCancel={() => setRestartOpen(false)}
        onConfirm={() => {
          void restartSteam();
        }}
      />

      <ConfirmModal
        open={modal !== null}
        title={
          modalResult === 'success'
            ? 'Installation complete'
            : modalResult === 'error'
              ? 'Installation failed'
              : modal?.kind === 'install-runtime'
                ? 'Install LumaForge runtime'
                : `Install ${modal?.kind === 'install-tool' ? modal.toolName : ''}`
        }
        description={
          modalResult === 'success'
            ? (progress?.message ?? 'Done.')
            : modalResult === 'error'
              ? (progress?.message ?? 'Something went wrong.')
              : (progress?.message ??
                'Downloads the release from GitHub, extracts it and deploys the files.')
        }
        confirmLabel={
          modalResult ? 'DONE' : modal?.kind === 'install-runtime' ? 'INSTALL' : 'INSTALL'
        }
        cancelLabel=""
        busyLabel="WORKING..."
        tone={modalResult === 'error' ? 'danger' : 'default'}
        busy={modalResult === null && modal !== null}
        closeOnBackdrop={modalResult !== null}
        closeOnEscape={modalResult !== null}
        autoFocus="confirm"
        onCancel={closeModal}
        onConfirm={closeModal}
      />
    </>
  );
}
