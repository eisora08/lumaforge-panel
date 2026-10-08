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
import { ThemesCard } from './ThemesCard';
import { useToast } from './Toast';
import { useI18n, type TranslateArg } from '../i18n';
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
  Trash2,
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

function stateBadge(
  t: (key: string, args?: readonly TranslateArg[]) => string,
  state: ComponentState
): { label: string; cls: string } {
  switch (state) {
    case 'enabled':
      return { label: t('badge.active'), cls: 'status-ok' };
    case 'disabled':
      return { label: t('badge.backedUp'), cls: 'status-label' };
    case 'missing':
      return { label: t('badge.notInstalled'), cls: 'status-warn' };
  }
}

export function DashboardView() {
  const toast = useToast();
  const { t } = useI18n();

  const [status, setStatus] = useState<PanelStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState<BusyKind>(null);
  const [modal, setModal] = useState<ModalKind>(null);
  const [progress, setProgress] = useState<InstallProgress | null>(null);
  const [modalResult, setModalResult] = useState<
    'success' | 'error' | null
  >(null);
  const [restartOpen, setRestartOpen] = useState(false);
  const [uninstallTarget, setUninstallTarget] = useState<ToolStatus | null>(
    null
  );

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

  const handleCdpToggle = useCallback(async () => {
    if (!status || busy || modal) return;
    const desired = status.cdp !== 'enabled';

    if (status.cdp === 'missing') {
      openInstallModal({ kind: 'install-runtime' });
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
  }, [status, busy, modal, openInstallModal, refresh, toast]);

  const handleToolToggle = useCallback(
    async (tool: ToolStatus) => {
      if (busy || modal) return;
      const desired = tool.state !== 'enabled';

      if (desired && !tool.installed) {
        openInstallModal({
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
    [busy, modal, openInstallModal, refresh, toast]
  );

  const handleUninstall = useCallback(async () => {
    const target = uninstallTarget;
    if (!target || busy || modal) return;

    setBusy('tool');
    try {
      const result = await invoke<PanelResult>('uninstall_tool', {
        toolId: target.id,
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
      setUninstallTarget(null);
      await refresh();
      if (pendingRestartRef.current) {
        pendingRestartRef.current = false;
        setRestartOpen(true);
      }
    }
  }, [uninstallTarget, busy, modal, refresh, toast]);

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
  const cdpLoader = status?.cdpLoader || 'wsock32.dll';

  const cdpStatusCopy = useMemo(() => {
    if (cdpBusy) {
      return {
        eyebrow: t('hero.busyEyebrow'),
        title: t('hero.busyTitle'),
        description: t('hero.busyDesc'),
        buttonLabel: t('hero.busyBtn'),
        description2: t('hero.busyDesc2'),
      };
    }
    if (loading && !status) {
      return {
        eyebrow: t('hero.checkEyebrow'),
        title: t('hero.checkTitle'),
        description: t('hero.checkDesc'),
        buttonLabel: t('hero.checkBtn'),
        description2: t('hero.checkDesc2'),
      };
    }
    if (cdpState === 'missing') {
      return {
        eyebrow: t('hero.missingEyebrow'),
        title: t('hero.missingTitle'),
        description: t('hero.missingDesc', [cdpLoader]),
        buttonLabel: t('hero.missingBtn'),
        description2: t('hero.missingDesc2'),
      };
    }
    if (cdpState === 'enabled') {
      return {
        eyebrow: t('hero.enabledEyebrow'),
        title: t('hero.enabledTitle'),
        description: t('hero.enabledDesc', [cdpLoader]),
        buttonLabel: t('hero.enabledBtn'),
        description2: t('hero.enabledDesc2'),
      };
    }
    return {
      eyebrow: t('hero.disabledEyebrow'),
      title: t('hero.disabledTitle'),
      description: t('hero.disabledDesc', [cdpLoader]),
      buttonLabel: t('hero.disabledBtn'),
      description2: t('hero.disabledDesc2'),
    };
  }, [cdpBusy, cdpLoader, cdpState, loading, status, t]);

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

  const steamStatusText = useMemo(() => {
    if (loading && !status) return t('steam.checking');
    if (status?.steam.steamRunning) return t('steam.running');
    return t('steam.stopped');
  }, [status, loading, t]);

  const openSteamAction = useMemo(() => {
    if (!status) return null;
    if (status.steam.steamRunning) return null;
    if (status.steam.steamExecutableFound) return t('steam.startSteam');
    return null;
  }, [status, t]);

  return (
    <>
      <div className="view-content panel-dashboard">
        <main className="panel-column">
          <section
            className={`panel-hero panel-hero--${heroState}`}
            aria-label={t('aria.cdpInjection')}
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
                  {t('fact.components')}{' '}
                  <strong>{stateBadge(t, cdpState).label}</strong>
                </span>
                <span className="panel-fact">
                  {t('fact.componentsOn')}{' '}
                  <strong>
                    {t('fact.count', [
                      enabledTools,
                      status?.tools.length ?? 0,
                    ])}
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
                disabled={cdpBusy || modal !== null || (loading && !status)}
                aria-pressed={cdpState === 'enabled'}
                aria-busy={cdpBusy}
                aria-label={
                  cdpState === 'enabled'
                    ? t('aria.disableCdp')
                    : t('aria.enableCdp')
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

          <section
            className="panel-components"
            aria-label={t('aria.components')}
          >
            {status?.tools.map((tool) => {
              const badge = stateBadge(t, tool.state);
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
                      disabled={toggleBusy || modal !== null}
                      onClick={() => {
                        openInstallModal({
                          kind: 'install-tool',
                          toolId: tool.id,
                          toolName: tool.name,
                        });
                      }}
                    >
                      {t('ui.update')}
                    </button>
                  ) : tool.state === 'missing' ? (
                    <button
                      type="button"
                      className="btn btn-secondary btn-sm panel-component-update"
                      disabled={toggleBusy || modal !== null}
                      onClick={() => {
                        openInstallModal({
                          kind: 'install-tool',
                          toolId: tool.id,
                          toolName: tool.name,
                        });
                      }}
                    >
                      {t('ui.install')}
                    </button>
                  ) : (
                    <span className="panel-component-version">
                      {tool.installed && tool.installedVersion
                        ? `v${tool.installedVersion}`
                        : ''}
                    </span>
                  )}

                  {tool.state !== 'missing' && (
                    <button
                      type="button"
                      className="panel-component-remove"
                      disabled={toggleBusy || modal !== null}
                      onClick={() => setUninstallTarget(tool)}
                      aria-label={t('aria.uninstall', [tool.name])}
                      title={t('aria.uninstall', [tool.name])}
                    >
                      <Trash2 size={13} aria-hidden="true" />
                    </button>
                  )}

                  <button
                    type="button"
                    className="toggle panel-component-toggle"
                    role="switch"
                    aria-checked={tool.state === 'enabled'}
                    aria-label={t('aria.toggleTool', [tool.name])}
                    aria-busy={toggleBusy}
                    disabled={toggleBusy || modal !== null || loading}
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

          <ThemesCard cdpState={cdpState} />

          <div className="panel-footer">
            <span className="panel-footer-path">
              {status?.steamRoot
                ? t('footer.steamRoot', [status.steamRoot])
                : t('footer.noSteamRoot')}
            </span>

            <button
              type="button"
              className="btn btn-secondary btn-sm"
              onClick={() => {
                void refresh();
              }}
              disabled={loading || busy !== null}
            >
              {t('ui.refresh')}
            </button>
          </div>
        </main>
      </div>

      <ConfirmModal
        open={restartOpen}
        title={t('restart.title')}
        description={t('restart.desc')}
        warning={t('restart.warning')}
        confirmLabel={t('restart.confirm')}
        cancelLabel={t('restart.cancel')}
        busyLabel={t('restart.busy')}
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
            ? t('install.complete')
            : modalResult === 'error'
              ? t('install.failed')
              : modal?.kind === 'install-runtime'
                ? t('install.runtimeTitle')
                : t('install.toolTitle', [
                    modal?.kind === 'install-tool' ? modal.toolName : '',
                  ])
        }
        description={
          modalResult === 'success'
            ? (progress?.message ?? t('install.doneMsg'))
            : modalResult === 'error'
              ? (progress?.message ?? t('install.wrong'))
              : (progress?.message ?? t('install.desc'))
        }
        confirmLabel={modalResult ? t('ui.done') : t('ui.install')}
        cancelLabel=""
        busyLabel={t('ui.working')}
        tone={modalResult === 'error' ? 'danger' : 'default'}
        busy={modalResult === null && modal !== null}
        closeOnBackdrop={modalResult !== null}
        closeOnEscape={modalResult !== null}
        autoFocus="confirm"
        onCancel={closeModal}
        onConfirm={closeModal}
      />

      <ConfirmModal
        open={uninstallTarget !== null}
        title={t('uninstall.title', [uninstallTarget?.name ?? ''])}
        description={t('uninstall.desc', [
          uninstallTarget?.deployPath ?? t('uninstall.disk'),
        ])}
        warning={t('uninstall.warning')}
        confirmLabel={t('uninstall.confirm')}
        cancelLabel={t('uninstall.cancel')}
        busyLabel={t('uninstall.busy')}
        tone="danger"
        busy={busy === 'tool'}
        autoFocus="cancel"
        onCancel={() => setUninstallTarget(null)}
        onConfirm={() => {
          void handleUninstall();
        }}
      />
    </>
  );
}
