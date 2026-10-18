import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { convertFileSrc, invoke } from '@tauri-apps/api/core';
import { useToast } from './Toast';
import { hasLaunchHint, useI18n } from '../i18n';
import {
  ChevronLeft,
  ChevronRight,
  Palette,
  RefreshCw,
  RotateCcw,
  SlidersHorizontal,
  TriangleAlert,
  X,
} from 'lucide-react';
import type {
  CatalogTheme,
  ComponentState,
  PanelResult,
  ThemeCondition,
  ThemeInfo,
  ThemesCatalog,
  ThemesState,
} from '../types';
import { ConfirmModal } from './ConfirmModal';

interface ThemesCardProps {
  cdpState: ComponentState;
}

function sliderNumeric(condition: ThemeCondition): number {
  const raw = condition.selected ?? String(condition.default ?? '');
  const match = String(raw).match(/-?\d+(\.\d+)?/);
  if (match) {
    const parsed = Number.parseFloat(match[0]);
    if (Number.isFinite(parsed)) return parsed;
  }
  return condition.slider?.min ?? 0;
}

function conditionValue(condition: ThemeCondition): string {
  if (condition.selected != null) return condition.selected;
  return String(condition.default ?? '');
}

/** A rail entry: a catalog theme (installed or not) or a local-only theme. */
type RailItem =
  | { kind: 'catalog'; entry: CatalogTheme; installed: ThemeInfo | null }
  | { kind: 'local'; theme: ThemeInfo };

function itemId(item: RailItem): string {
  return item.kind === 'catalog' ? item.entry.id : item.theme.id;
}

function itemName(item: RailItem): string {
  return item.kind === 'catalog' ? item.entry.name : item.theme.name;
}

function itemInstalled(item: RailItem): boolean {
  return item.kind === 'local' || item.installed !== null;
}

function itemTheme(item: RailItem | null): ThemeInfo | null {
  if (!item) return null;
  return item.kind === 'local' ? item.theme : item.installed;
}

export function ThemesCard({ cdpState }: ThemesCardProps) {
  const toast = useToast();
  const { t } = useI18n();

  const [state, setState] = useState<ThemesState | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [imageErrors, setImageErrors] = useState<Record<string, boolean>>({});
  const [sliderDrafts, setSliderDrafts] = useState<Record<string, string>>(
    {}
  );
  const [panelOpen, setPanelOpen] = useState(false);
  const [catalog, setCatalog] = useState<ThemesCatalog | null>(null);
  const [previewPaths, setPreviewPaths] = useState<Record<string, string>>(
    {}
  );
  const [busyId, setBusyId] = useState<string | null>(null);
  const [uninstallTarget, setUninstallTarget] = useState<{
    id: string;
    name: string;
  } | null>(null);
  const [retrying, setRetrying] = useState(false);

  const railRef = useRef<HTMLDivElement | null>(null);
  const sliderTimer = useRef<number | null>(null);
  const previewPending = useRef<Set<string>>(new Set());

  const refresh = useCallback(async (forceCatalog = false) => {
    try {
      const [next, cat] = await Promise.all([
        invoke<ThemesState>('themes_get'),
        invoke<ThemesCatalog>('themes_catalog', {
          force: forceCatalog,
        }).catch(() => null),
      ]);
      setState(next);
      if (cat) setCatalog(cat);
      setSelectedId(
        (prev) =>
          prev ??
          (next.active || next.themes[0]?.id || cat?.entries[0]?.id || null)
      );
    } catch (error) {
      console.error('[lumaforge-panel] themes_get failed:', error);
    } finally {
      setLoading(false);
    }
  }, []);

  const loadCatalog = useCallback(async (force: boolean) => {
    try {
      const cat = await invoke<ThemesCatalog>('themes_catalog', { force });
      setCatalog(cat);
    } catch {
      setCatalog({ entries: [], source: 'empty' });
    }
  }, []);

  const handleRetry = useCallback(async () => {
    if (retrying) return;
    setRetrying(true);
    try {
      await loadCatalog(true);
    } finally {
      setRetrying(false);
    }
  }, [loadCatalog, retrying]);

  useEffect(() => {
    void refresh();
    return () => {
      if (sliderTimer.current) window.clearTimeout(sliderTimer.current);
    };
  }, [refresh]);

  // Cache catalog preview images on disk (one request per image; the panel
  // serves the local copy back through the asset protocol).
  useEffect(() => {
    if (!catalog) return;
    for (const entry of catalog.entries) {
      if (
        !entry.preview ||
        entry.previewPath ||
        previewPaths[entry.id] ||
        previewPending.current.has(entry.id)
      ) {
        continue;
      }
      previewPending.current.add(entry.id);
      void invoke<string>('theme_fetch_preview', { url: entry.preview })
        .then((path) => {
          setPreviewPaths((prev) => ({ ...prev, [entry.id]: path }));
        })
        .catch(() => {
          // Placeholder stays; retried on the next catalog refresh.
        })
        .finally(() => {
          previewPending.current.delete(entry.id);
        });
    }
  }, [catalog, previewPaths]);

  // Close the settings panel on Escape.
  useEffect(() => {
    if (!panelOpen) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') setPanelOpen(false);
    };
    document.addEventListener('keydown', onKeyDown);
    return () => document.removeEventListener('keydown', onKeyDown);
  }, [panelOpen]);

  const run = useCallback(
    async (
      action: () => Promise<PanelResult>,
      opts?: { hint?: boolean; silent?: boolean }
    ) => {
      if (!opts?.silent) setBusy(true);
      try {
        const result = await action();
        if (result.ok) {
          if (!opts?.silent) {
            const needsHint =
              opts?.hint && !hasLaunchHint(result.message);
            toast.success(
              needsHint
                ? `${result.message}${t('themes.hint')}`
                : result.message
            );
          }
        } else {
          toast.error(result.message);
        }
        return result.ok;
      } catch (error) {
        toast.error(
          error instanceof Error ? error.message : String(error)
        );
        return false;
      } finally {
        if (!opts?.silent) setBusy(false);
        await refresh();
      }
    },
    [refresh, toast]
  );

  const items = useMemo<RailItem[]>(() => {
    const installed = state?.themes ?? [];
    const entries = catalog?.entries ?? [];
    const byId = new Map(installed.map((theme) => [theme.id, theme]));
    const listed = new Set(entries.map((entry) => entry.id));
    const merged: RailItem[] = entries.map((entry) => ({
      kind: 'catalog',
      entry,
      installed: byId.get(entry.id) ?? null,
    }));
    for (const theme of installed) {
      if (!listed.has(theme.id)) merged.push({ kind: 'local', theme });
    }
    return merged;
  }, [state, catalog]);

  const activeTheme = state?.themes.find((t) => t.id === state.active) ?? null;
  const selectedItem =
    items.find((item) => itemId(item) === selectedId) ?? items[0] ?? null;
  const selectedTheme = itemTheme(selectedItem);
  const selectedInstalled = selectedItem ? itemInstalled(selectedItem) : false;
  const count = state?.themes.length ?? 0;
  const themingOn = (state?.active ?? '') !== '';
  const busyAll = busy || busyId !== null;

  // Track the rail scroll bounds so the arrow buttons disable at each end.
  const [railEnds, setRailEnds] = useState({ start: true, end: true });

  useEffect(() => {
    const el = railRef.current;
    if (!el) {
      return;
    }
    const update = () => {
      const max = el.scrollWidth - el.clientWidth;
      setRailEnds({
        start: el.scrollLeft <= 1,
        end: max <= 0 || el.scrollLeft >= max - 1,
      });
    };
    update();
    el.addEventListener('scroll', update, { passive: true });
    const observer = new ResizeObserver(update);
    observer.observe(el);
    window.addEventListener('resize', update);
    return () => {
      el.removeEventListener('scroll', update);
      observer.disconnect();
      window.removeEventListener('resize', update);
    };
  }, [items.length]);

  const conditionGroups = useMemo(() => {
    if (!selectedTheme) return [] as { label: string; items: ThemeCondition[] }[];
    const groups = new Map<string, ThemeCondition[]>();
    for (const condition of selectedTheme.conditions) {
      const parts = [condition.tab, condition.section].filter(Boolean);
      const label = parts.length > 0 ? parts.join(' / ') : t('themes.conditionsLabel');
      const list = groups.get(label) ?? [];
      list.push(condition);
      groups.set(label, list);
    }
    return Array.from(groups.entries()).map(([label, items]) => ({
      label,
      items,
    }));
  }, [selectedTheme, t]);

  const handleMasterToggle = useCallback(async () => {
    if (busy || !state) return;
    if (themingOn) {
      await run(() => invoke<PanelResult>('theme_deactivate'), {
        hint: true,
      });
      return;
    }
    if (!selectedItem || !itemInstalled(selectedItem)) {
      toast.info(
        items.some((item) => itemInstalled(item))
          ? t('themes.selectInstalled')
          : t('themes.noInstalled')
      );
      return;
    }
    await run(
      () => invoke<PanelResult>('theme_activate', { id: itemId(selectedItem) }),
      { hint: true }
    );
  }, [busy, items, run, selectedItem, state, themingOn, t, toast]);

  const handleActivate = useCallback(async () => {
    if (!selectedTheme) return;
    await run(
      () => invoke<PanelResult>('theme_activate', { id: selectedTheme.id }),
      { hint: true }
    );
  }, [run, selectedTheme]);

  const handleInstall = useCallback(
    async (id: string) => {
      if (busy || busyId) return;
      setBusyId(id);
      try {
        const ok = await run(() => invoke<PanelResult>('theme_install', { id }));
        if (ok) setSelectedId(id);
      } finally {
        setBusyId(null);
      }
    },
    [busy, busyId, run]
  );

  const handleUninstall = useCallback(async () => {
    const target = uninstallTarget;
    if (!target || busyId) return;
    setBusyId(target.id);
    try {
      const ok = await run(() =>
        invoke<PanelResult>('theme_uninstall', { id: target.id })
      );
      if (ok) {
        setUninstallTarget(null);
        if (selectedId === target.id) setSelectedId(null);
      }
    } finally {
      setBusyId(null);
    }
  }, [busyId, run, selectedId, uninstallTarget]);

  const handleSelectCondition = useCallback(
    (condition: ThemeCondition, value: string) => {
      if (!selectedTheme) return;
      setSliderDrafts((prev) => {
        const next = { ...prev };
        delete next[`${selectedTheme.id}:${condition.key}`];
        return next;
      });
      void run(
        () =>
          invoke<PanelResult>('theme_set_condition', {
            themeId: selectedTheme.id,
            key: condition.key,
            value,
          }),
        { silent: true }
      );
    },
    [run, selectedTheme]
  );

  const handleSlider = useCallback(
    (condition: ThemeCondition, value: string) => {
      if (!selectedTheme) return;
      const token = `${selectedTheme.id}:${condition.key}`;
      setSliderDrafts((prev) => ({ ...prev, [token]: value }));
      if (sliderTimer.current) window.clearTimeout(sliderTimer.current);
      sliderTimer.current = window.setTimeout(() => {
        void run(
          () =>
            invoke<PanelResult>('theme_set_condition', {
              themeId: selectedTheme.id,
              key: condition.key,
              value,
            }),
          { silent: true }
        );
      }, 400);
    },
    [run, selectedTheme]
  );

  const handleResetCondition = useCallback(
    (condition: ThemeCondition) => {
      if (!selectedTheme) return;
      setSliderDrafts((prev) => {
        const next = { ...prev };
        delete next[`${selectedTheme.id}:${condition.key}`];
        return next;
      });
      void run(
        () =>
          invoke<PanelResult>('theme_reset_condition', {
            themeId: selectedTheme.id,
            key: condition.key,
          }),
        { silent: true }
      );
    },
    [run, selectedTheme]
  );

  const scrollRail = useCallback((direction: -1 | 1) => {
    railRef.current?.scrollBy({
      left: direction * 340,
      behavior: 'smooth',
    });
  }, []);

  const showCdpHint = cdpState !== 'enabled';

  return (
    <>
      <section className="panel-themes" aria-label={t('themes.title')}>
      <div className="panel-themes-head">
        <span className="panel-component-icon" aria-hidden="true">
          <Palette size={16} />
        </span>

        <strong className="panel-component-name">{t('themes.title')}</strong>

        {loading ? (
          <span className="status-tag status-label">{t('themes.loadingTag')}</span>
        ) : themingOn && activeTheme ? (
          <span className="status-tag status-ok">{activeTheme.name}</span>
        ) : (
          <span className="status-tag status-label">{t('themes.offTag')}</span>
        )}

        <span className="panel-themes-count">
          {t('themes.count', [count])}
        </span>

        <div className="panel-themes-head-actions">
          {selectedItem && !selectedInstalled ? (
            <button
              type="button"
              className="btn btn-primary btn-sm panel-themes-apply"
              onClick={() => {
                void handleInstall(itemId(selectedItem));
              }}
              disabled={loading || busyAll}
            >
              {busyId && itemId(selectedItem) === busyId
                ? t('themes.installing')
                : t('themes.installBtn')}
            </button>
          ) : (
            <button
              type="button"
              className="btn btn-primary btn-sm panel-themes-apply"
              onClick={() => {
                void handleActivate();
              }}
              disabled={
                busyAll ||
                loading ||
                !selectedTheme ||
                selectedTheme.id === state?.active
              }
            >
              {selectedTheme && selectedTheme.id === state?.active
                ? t('themes.activeBtn')
                : t('themes.applyBtn')}
            </button>
          )}

          {selectedItem && selectedInstalled && (
            <button
              type="button"
              className="btn btn-secondary btn-sm panel-themes-uninstall"
              onClick={() =>
                setUninstallTarget({
                  id: itemId(selectedItem),
                  name: itemName(selectedItem),
                })
              }
              disabled={loading || busyAll}
            >
              {t('themes.uninstallBtn')}
            </button>
          )}

          <button
            type="button"
            className="panel-themes-icon-btn"
            onClick={() => setPanelOpen(true)}
            disabled={loading || !selectedTheme}
            aria-label={t('aria.themeSettings')}
            aria-expanded={panelOpen}
            title={t('aria.themeSettings')}
          >
            <SlidersHorizontal size={13} aria-hidden="true" />
          </button>

          <button
            type="button"
            className="panel-themes-icon-btn"
            onClick={() => {
              void refresh(true);
            }}
            disabled={loading || busyAll}
            aria-label={t('aria.refreshThemes')}
            title={t('ui.refresh')}
          >
            <RefreshCw size={13} aria-hidden="true" />
          </button>

          <button
            type="button"
            className="toggle panel-themes-toggle"
            role="switch"
            aria-checked={themingOn}
            aria-label={themingOn ? t('aria.disableTheming') : t('aria.enableTheming')}
            aria-busy={busyAll}
            disabled={busyAll || loading}
            onClick={() => {
              void handleMasterToggle();
            }}
          >
            <span className="toggle-track" />
          </button>
        </div>
      </div>

      {showCdpHint && !loading && (
        <p className="panel-themes-note">
          {t('themes.cdpNote')}
        </p>
      )}

      {loading ? (
        <div className="panel-themes-empty">{t('themes.loadingMsg')}</div>
      ) : items.length === 0 && catalog?.source !== 'empty' ? (
        <div className="panel-themes-empty">{t('themes.noInstalled')}</div>
      ) : (
        <>
          <div className="panel-themes-rail-wrap">
            <button
              type="button"
              className="panel-themes-rail-btn"
              onClick={() => scrollRail(-1)}
              disabled={railEnds.start}
              aria-label={t('aria.scrollLeft')}
            >
              <ChevronLeft size={14} aria-hidden="true" />
            </button>

            <div className="panel-themes-rail" ref={railRef}>
              {catalog?.source === 'empty' && (
                <div className="panel-theme-card panel-theme-card--error">
                  <TriangleAlert size={18} aria-hidden="true" />
                  <span className="panel-theme-card-error-text">
                    {t('themes.catalogError')}
                  </span>
                  <button
                    type="button"
                    className="btn btn-secondary btn-sm"
                    onClick={() => {
                      void handleRetry();
                    }}
                    disabled={retrying}
                  >
                    {retrying ? t('themes.retrying') : t('themes.retry')}
                  </button>
                </div>
              )}

              {items.map((item) => {
                const id = itemId(item);
                const isActive = (state?.active ?? '') === id;
                const isSelected = selectedItem
                  ? itemId(selectedItem) === id
                  : false;
                const installed = itemInstalled(item);
                const broken = imageErrors[id];
                const preview =
                  item.kind === 'local'
                    ? item.theme.previewPath ?? null
                    : (item.installed?.previewPath ??
                      item.entry.previewPath ??
                      previewPaths[item.entry.id] ??
                      null);
                const meta =
                  item.kind === 'local'
                    ? item.theme.author ||
                      (item.theme.version ? `v${item.theme.version}` : '') ||
                      '—'
                    : item.entry.version
                      ? `v${item.entry.version}`
                      : `${item.entry.owner}/${item.entry.repo}`;
                return (
                  <div
                    role="button"
                    tabIndex={0}
                    key={id}
                    className={[
                      'panel-theme-card',
                      isSelected ? 'panel-theme-card--selected' : '',
                      isActive ? 'panel-theme-card--active' : '',
                      !installed ? 'panel-theme-card--not-installed' : '',
                    ]
                      .filter(Boolean)
                      .join(' ')}
                    aria-pressed={isSelected}
                    onClick={() => setSelectedId(id)}
                    onKeyDown={(event) => {
                      if (event.key === 'Enter' || event.key === ' ') {
                        event.preventDefault();
                        setSelectedId(id);
                      }
                    }}
                  >
                    <span className="panel-theme-card-thumb">
                      {preview && !broken ? (
                        <img
                          src={convertFileSrc(preview)}
                          alt=""
                          loading="lazy"
                          onError={() =>
                            setImageErrors((prev) => ({
                              ...prev,
                              [id]: true,
                            }))
                          }
                        />
                      ) : (
                        <span
                          className="panel-theme-card-fallback"
                          aria-hidden="true"
                        >
                          <Palette size={18} />
                        </span>
                      )}

                      {!installed && (
                        <span className="panel-theme-card-install">
                          <button
                            type="button"
                            className="btn btn-primary btn-sm"
                            onClick={(event) => {
                              event.stopPropagation();
                              void handleInstall(id);
                            }}
                            disabled={loading || busyAll}
                          >
                            {busyId === id
                              ? t('themes.installing')
                              : t('themes.installBtn')}
                          </button>
                        </span>
                      )}

                      {isActive && (
                        <span className="panel-theme-card-badge">
                          {t('themes.activeBadge')}
                        </span>
                      )}
                    </span>

                    <span className="panel-theme-card-name">
                      {itemName(item)}
                    </span>
                    <span className="panel-theme-card-meta">{meta}</span>
                  </div>
                );
              })}
            </div>

            <button
              type="button"
              className="panel-themes-rail-btn"
              onClick={() => scrollRail(1)}
              disabled={railEnds.end}
              aria-label={t('aria.scrollRight')}
            >
              <ChevronRight size={14} aria-hidden="true" />
            </button>
          </div>
        </>
      )}
      </section>

      {panelOpen && selectedTheme && (
        <div className="theme-panel-layer">
          <div
            className="theme-panel-backdrop"
            onClick={() => setPanelOpen(false)}
            aria-hidden="true"
          />

          <aside
            className="theme-panel"
            role="dialog"
            aria-modal="true"
            aria-label={`${t('aria.themeSettings')} — ${selectedTheme.name}`}
          >
            <header className="theme-panel-head">
              <span className="panel-component-icon" aria-hidden="true">
                <SlidersHorizontal size={15} />
              </span>
              <div className="theme-panel-title">
                <strong>{selectedTheme.name}</strong>
                <span>
                  {selectedTheme.id === state?.active
                    ? t('panel.activeTheme')
                    : t('panel.settings')}
                  {conditionGroups.length > 0
                    ? t('panel.conditionsCount', [
                        selectedTheme.conditions.length,
                      ])
                    : ''}
                </span>
              </div>
              <button
                type="button"
                className="panel-themes-icon-btn"
                onClick={() => setPanelOpen(false)}
                aria-label={t('aria.closeThemeSettings')}
                title={t('panel.closeEsc')}
              >
                <X size={13} aria-hidden="true" />
              </button>
            </header>

            <div className="theme-panel-body">
              {selectedTheme.description && (
                <p className="theme-panel-desc">
                  {selectedTheme.description}
                </p>
              )}

              {conditionGroups.length === 0 ? (
                <p className="panel-themes-empty">
                  {t('panel.noConditions')}
                </p>
              ) : (
                <div className="panel-themes-conditions">
                  {conditionGroups.map((group) => (
                    <div
                      className="panel-themes-cond-group"
                      key={group.label}
                    >
                      <span className="panel-themes-cond-label">
                        {group.label}
                      </span>

                      {group.items.map((condition) => {
                        const token = `${selectedTheme.id}:${condition.key}`;
                        const draft = sliderDrafts[token];
                        const overridden = condition.selected != null;

                        if (condition.slider) {
                          const numeric =
                            draft != null
                              ? Number.parseFloat(draft)
                              : sliderNumeric(condition);
                          const value = Number.isFinite(numeric)
                            ? numeric
                            : condition.slider.min;
                          return (
                            <div
                              className="panel-themes-cond-row"
                              key={condition.key}
                            >
                              <span className="panel-themes-cond-name">
                                {condition.description || condition.key}
                              </span>
                              <input
                                type="range"
                                className="panel-themes-slider"
                                min={condition.slider.min}
                                max={condition.slider.max}
                                step={
                                  condition.slider.step > 0
                                    ? condition.slider.step
                                    : 1
                                }
                                value={value}
                                disabled={busy}
                                aria-label={
                                  condition.description || condition.key
                                }
                                onChange={(e) =>
                                  handleSlider(condition, e.target.value)
                                }
                              />
                              <span className="panel-themes-cond-value">
                                {value}
                                {condition.slider.unit}
                              </span>
                              {overridden && (
                                <button
                                  type="button"
                                  className="panel-themes-reset"
                                  onClick={() =>
                                    handleResetCondition(condition)
                                  }
                                  disabled={busy}
                                  aria-label={t('aria.resetCondition', [condition.key])}
                                  title={t('resetToDefault')}
                                >
                                  <RotateCcw size={12} aria-hidden="true" />
                                </button>
                              )}
                            </div>
                          );
                        }

                        const current =
                          draft ?? conditionValue(condition) ?? '';
                        return (
                          <div
                            className="panel-themes-cond-row"
                            key={condition.key}
                          >
                            <span className="panel-themes-cond-name">
                              {condition.description || condition.key}
                            </span>
                            <select
                              className="panel-themes-select"
                              value={current}
                              disabled={busy}
                              aria-label={condition.description || condition.key}
                              onChange={(e) =>
                                handleSelectCondition(condition, e.target.value)
                              }
                            >
                              {(condition.values ?? []).map((option) => (
                                <option key={option} value={option}>
                                  {option}
                                </option>
                              ))}
                              {!condition.values?.includes(current) && (
                                <option value={current}>{current}</option>
                              )}
                            </select>
                            {overridden && (
                              <button
                                type="button"
                                className="panel-themes-reset"
                                onClick={() =>
                                  handleResetCondition(condition)
                                }
                                disabled={busy}
                                aria-label={t('aria.resetCondition', [
                                  condition.key,
                                ])}
                                title={t('resetToDefault')}
                              >
                                <RotateCcw size={12} aria-hidden="true" />
                              </button>
                            )}
                          </div>
                        );
                      })}
                    </div>
                  ))}
                </div>
              )}
            </div>

            <footer className="theme-panel-foot">
              <span>{t('panel.footer')}</span>
              <button
                type="button"
                className="btn btn-secondary btn-sm"
                onClick={() => setPanelOpen(false)}
              >
                {t('ui.done')}
              </button>
            </footer>
          </aside>
        </div>
      )}

      <ConfirmModal
        open={uninstallTarget !== null}
        title={t('uninstall.title', [uninstallTarget?.name ?? ''])}
        description={t('themes.uninstallDesc')}
        warning={t('themes.uninstallWarning')}
        confirmLabel={t('uninstall.confirm')}
        cancelLabel={t('uninstall.cancel')}
        busyLabel={t('uninstall.busy')}
        tone="danger"
        busy={busyId !== null}
        autoFocus="cancel"
        onCancel={() => setUninstallTarget(null)}
        onConfirm={() => {
          void handleUninstall();
        }}
      />
    </>
  );
}
