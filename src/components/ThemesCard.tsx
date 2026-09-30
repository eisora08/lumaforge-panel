import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { convertFileSrc, invoke } from '@tauri-apps/api/core';
import { useToast } from './Toast';
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

const HINT = ' — restart Steam if it doesn\u2019t look right';

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
              opts?.hint && !/launches/i.test(result.message);
            toast.success(
              needsHint ? `${result.message}${HINT}` : result.message
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

  const conditionGroups = useMemo(() => {
    if (!selectedTheme) return [] as { label: string; items: ThemeCondition[] }[];
    const groups = new Map<string, ThemeCondition[]>();
    for (const condition of selectedTheme.conditions) {
      const parts = [condition.tab, condition.section].filter(Boolean);
      const label = parts.length > 0 ? parts.join(' / ') : 'Conditions';
      const list = groups.get(label) ?? [];
      list.push(condition);
      groups.set(label, list);
    }
    return Array.from(groups.entries()).map(([label, items]) => ({
      label,
      items,
    }));
  }, [selectedTheme]);

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
      toast.info('No themes installed');
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
      <section className="panel-themes" aria-label="Themes">
      <div className="panel-themes-head">
        <span className="panel-component-icon" aria-hidden="true">
          <Palette size={16} />
        </span>

        <strong className="panel-component-name">Themes</strong>

        {loading ? (
          <span className="status-tag status-label">LOADING</span>
        ) : themingOn && activeTheme ? (
          <span className="status-tag status-ok">{activeTheme.name}</span>
        ) : (
          <span className="status-tag status-label">OFF</span>
        )}

        <span className="panel-themes-count">
          {count} installed
        </span>

        <div className="panel-themes-head-actions">
          <button
            type="button"
            className="panel-themes-icon-btn"
            onClick={() => {
              void refresh();
            }}
            disabled={loading || busy}
            aria-label="Refresh themes"
            title="Refresh"
          >
            <RefreshCw size={13} aria-hidden="true" />
          </button>

          <button
            type="button"
            className="toggle panel-themes-toggle"
            role="switch"
            aria-checked={themingOn}
            aria-label={themingOn ? 'Disable theming' : 'Enable theming'}
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
          CDP injection is off — enable it so theme changes reach Steam live.
        </p>
      )}

      {loading ? (
        <div className="panel-themes-empty">Loading themes…</div>
      ) : count === 0 ? (
        <div className="panel-themes-empty">
          No Steam themes found. Install themes into{' '}
          <code>LumaForge/themes</code>.
        </div>
      ) : (
        <>
          <div className="panel-themes-rail-wrap">
            <button
              type="button"
              className="panel-themes-rail-btn"
              onClick={() => scrollRail(-1)}
              aria-label="Scroll themes left"
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
                        <span className="panel-theme-card-badge">Active</span>
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
              aria-label="Scroll themes right"
            >
              <ChevronRight size={14} aria-hidden="true" />
            </button>
          </div>

          {selectedTheme && (
            <div className="panel-themes-detail">
              <div className="panel-themes-detail-head">
                <div className="panel-themes-detail-copy">
                  <span className="panel-themes-detail-title">
                    {selectedTheme.name}
                    {selectedTheme.id === state?.active && (
                      <span className="status-tag status-ok panel-themes-detail-chip">
                        ACTIVE
                      </span>
                    )}
                  </span>
                  <span className="panel-themes-detail-sub">
                    {[
                      selectedTheme.author ? `by ${selectedTheme.author}` : '',
                      selectedTheme.description,
                    ]
                      .filter(Boolean)
                      .join(' · ') || 'No description'}
                  </span>
                </div>

                <button
                  type="button"
                  className="btn btn-primary btn-sm"
                  onClick={() => {
                    void handleActivate();
                  }}
                  disabled={
                    busy ||
                    loading ||
                    selectedTheme.id === state?.active
                  }
                >
                  {selectedTheme.id === state?.active ? 'ACTIVE' : 'APPLY'}
                </button>

                <button
                  type="button"
                  className="panel-themes-icon-btn panel-themes-gear"
                  onClick={() => setPanelOpen(true)}
                  disabled={loading}
                  aria-label="Theme settings"
                  aria-expanded={panelOpen}
                  title="Theme settings"
                >
                  <SlidersHorizontal size={13} aria-hidden="true" />
                </button>
              </div>
            </div>
          )}
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
            aria-label={`Theme settings — ${selectedTheme.name}`}
          >
            <header className="theme-panel-head">
              <span className="panel-component-icon" aria-hidden="true">
                <SlidersHorizontal size={15} />
              </span>
              <div className="theme-panel-title">
                <strong>{selectedTheme.name}</strong>
                <span>
                  {selectedTheme.id === state?.active
                    ? 'Active theme'
                    : 'Theme settings'}
                  {conditionGroups.length > 0
                    ? ` · ${selectedTheme.conditions.length} conditions`
                    : ''}
                </span>
              </div>
              <button
                type="button"
                className="panel-themes-icon-btn"
                onClick={() => setPanelOpen(false)}
                aria-label="Close theme settings"
                title="Close (Esc)"
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
                  This theme has no configurable conditions.
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
                                  aria-label={`Reset ${condition.key}`}
                                  title="Reset to theme default"
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
                                aria-label={`Reset ${condition.key}`}
                                title="Reset to theme default"
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
              <span>Changes apply live — restart Steam if it doesn’t look right.</span>
              <button
                type="button"
                className="btn btn-secondary btn-sm"
                onClick={() => setPanelOpen(false)}
              >
                DONE
              </button>
            </footer>
          </aside>
        </div>
      )}
    </>
  );
}
