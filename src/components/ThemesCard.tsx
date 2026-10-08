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
  X,
} from 'lucide-react';
import type {
  ComponentState,
  PanelResult,
  ThemeCondition,
  ThemesState,
} from '../types';

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

  const railRef = useRef<HTMLDivElement | null>(null);
  const sliderTimer = useRef<number | null>(null);

  const refresh = useCallback(async () => {
    try {
      const next = await invoke<ThemesState>('themes_get');
      setState(next);
      setSelectedId(
        (prev) => prev ?? (next.active || next.themes[0]?.id || null)
      );
    } catch (error) {
      console.error('[lumaforge-panel] themes_get failed:', error);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
    return () => {
      if (sliderTimer.current) window.clearTimeout(sliderTimer.current);
    };
  }, [refresh]);

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

  const activeTheme = state?.themes.find((t) => t.id === state.active) ?? null;
  const selectedTheme =
    state?.themes.find((t) => t.id === selectedId) ?? activeTheme ?? null;
  const count = state?.themes.length ?? 0;
  const themingOn = (state?.active ?? '') !== '';

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
  }, [count]);

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
    const target = selectedId ?? state.themes[0]?.id;
    if (!target) {
      toast.info(t('themes.noInstalled'));
      return;
    }
    await run(() => invoke<PanelResult>('theme_activate', { id: target }), {
      hint: true,
    });
  }, [busy, run, selectedId, state, themingOn, toast]);

  const handleActivate = useCallback(async () => {
    if (!selectedTheme) return;
    await run(
      () => invoke<PanelResult>('theme_activate', { id: selectedTheme.id }),
      { hint: true }
    );
  }, [run, selectedTheme]);

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
          <button
            type="button"
            className="btn btn-primary btn-sm panel-themes-apply"
            onClick={() => {
              void handleActivate();
            }}
            disabled={
              busy ||
              loading ||
              !selectedTheme ||
              selectedTheme.id === state?.active
            }
          >
            {selectedTheme && selectedTheme.id === state?.active
              ? t('themes.activeBtn')
              : t('themes.applyBtn')}
          </button>

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
              void refresh();
            }}
            disabled={loading || busy}
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
            aria-busy={busy}
            disabled={busy || loading}
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
      ) : count === 0 ? (
        <div className="panel-themes-empty">
          {t('themes.emptyBefore')}{' '}
          <code>LumaForge/themes</code>{t('themes.emptyAfter')}
        </div>
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
              {state?.themes.map((theme) => {
                const isActive = theme.id === state.active;
                const isSelected = theme.id === selectedTheme?.id;
                const broken = imageErrors[theme.id];
                return (
                  <button
                    type="button"
                    key={theme.id}
                    className={[
                      'panel-theme-card',
                      isSelected ? 'panel-theme-card--selected' : '',
                      isActive ? 'panel-theme-card--active' : '',
                    ]
                      .filter(Boolean)
                      .join(' ')}
                    aria-pressed={isSelected}
                    onClick={() => setSelectedId(theme.id)}
                  >
                    <span className="panel-theme-card-thumb">
                      {theme.previewPath && !broken ? (
                        <img
                          src={convertFileSrc(theme.previewPath)}
                          alt=""
                          loading="lazy"
                          onError={() =>
                            setImageErrors((prev) => ({
                              ...prev,
                              [theme.id]: true,
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
                      {isActive && (
                        <span className="panel-theme-card-badge">{t('themes.activeBadge')}</span>
                      )}
                    </span>

                    <span className="panel-theme-card-name">
                      {theme.name}
                    </span>
                    <span className="panel-theme-card-meta">
                      {theme.author ||
                        (theme.version ? `v${theme.version}` : '') ||
                        '—'}
                    </span>
                  </button>
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
    </>
  );
}
