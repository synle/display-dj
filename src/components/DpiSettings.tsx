import { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { DpiDisplay, MonitorMetadata } from '../types';
import Dropdown from './Dropdown';
import Tooltip from './Tooltip';

/** Absolute lowest DPI scale percent (mirrors backend `DPI_ABSOLUTE_MIN`). */
export const DPI_ABSOLUTE_MIN = 50;
/** Absolute highest DPI scale percent (mirrors backend `DPI_ABSOLUTE_MAX`). */
export const DPI_ABSOLUTE_MAX = 500;
/** Step interval bounds (mirror backend `DPI_STEP_MIN` / `DPI_STEP_MAX`). */
export const DPI_STEP_MIN = 1;
export const DPI_STEP_MAX = 100;
/** Delay before re-anchoring the popup after a scale change; the OS relayouts asynchronously. */
const REANCHOR_DELAY_MS = [400, 1500];

/**
 * Validates a min/max/step text triple.
 *
 * @param minText - Raw min input.
 * @param maxText - Raw max input.
 * @param stepText - Raw step input.
 * @returns Parsed `{ min, max, step }` or an error message.
 */
export function validateDpiRange(
  minText: string,
  maxText: string,
  stepText = '5',
): { min: number; max: number; step: number } | { error: string } {
  const isInt = (t: string) => /^\d+$/.test(t.trim());
  if (!isInt(minText) || !isInt(maxText) || !isInt(stepText))
    return { error: 'Min, max, and step must be whole numbers.' };
  const min = Number(minText);
  const max = Number(maxText);
  const step = Number(stepText);
  const inBand = (v: number) => v >= DPI_ABSOLUTE_MIN && v <= DPI_ABSOLUTE_MAX;
  if (!inBand(min) || !inBand(max)) {
    return { error: `Values must be between ${DPI_ABSOLUTE_MIN}% and ${DPI_ABSOLUTE_MAX}%.` };
  }
  if (min > max) return { error: 'Min must not exceed max.' };
  if (step < DPI_STEP_MIN || step > DPI_STEP_MAX)
    return { error: `Step must be between ${DPI_STEP_MIN}% and ${DPI_STEP_MAX}%.` };
  return { min, max, step };
}

/**
 * Scale choices for one display inside the user band.
 *
 * Discrete backends (macOS modes, Windows OS steps) return the OS list as-is —
 * the user band is ignored. Continuous backends (Linux/xrandr) get a generated
 * `min..=max` grid at `step`. The current value is always kept.
 *
 * @returns Ascending, de-duplicated percents.
 */
export function dpiOptionsFor(d: DpiDisplay, min: number, max: number, step: number): number[] {
  const base = d.continuous
    ? Array.from({ length: Math.floor((max - min) / step) + 1 }, (_, i) => min + i * step)
    : d.options;
  const all = d.current == null ? base : [...base, d.current];
  return [...new Set(all)].sort((a, b) => a - b);
}

/** Normalizes a display name for loose matching. */
const norm = (s: string) => s.toLowerCase().replace(/[^a-z0-9]/g, '');

/**
 * Pairs every monitor config with a DPI display. Hardware-ID matches are
 * claimed first for all rows so a loose name match on an earlier row can't
 * steal a later row's exact display.
 *
 * @returns Map of monitor uid → DPI display, plus the unmatched DPI displays.
 */
export function pairDpiDisplays(
  configs: MonitorMetadata[],
  displays: DpiDisplay[],
): { matches: Map<string, DpiDisplay>; unmatched: DpiDisplay[] } {
  const matches = new Map<string, DpiDisplay>();
  const taken = new Set<string>();
  const claim = (meta: MonitorMetadata, d: DpiDisplay | undefined) => {
    if (!d || matches.has(meta.uid)) return;
    matches.set(meta.uid, d);
    taken.add(d.id);
  };
  for (const meta of configs) {
    const apiName = (meta.apiName ?? '').toUpperCase();
    claim(
      meta,
      displays.find(
        (d) =>
          !taken.has(d.id) && !!d.hardwareId && apiName.includes(`(${d.hardwareId.toUpperCase()})`),
      ),
    );
  }
  for (const meta of configs) {
    if (!matches.has(meta.uid)) claim(meta, matchDpiDisplay(meta, displays, taken));
  }
  return { matches, unmatched: displays.filter((d) => !taken.has(d.id)) };
}

/**
 * Finds the DPI display matching a monitor config: hardware ID (e.g. the
 * `ACR0D1D` in `Generic PnP Monitor (ACR0D1D)`), then exact name, then
 * built-in ↔ built-in, then substring either way. Each DPI display matches once,
 * so the Settings list dedupes to the same displays as the main screen.
 *
 * @returns The matched display, or undefined.
 */
export function matchDpiDisplay(
  meta: MonitorMetadata,
  displays: DpiDisplay[],
  taken: Set<string>,
): DpiDisplay | undefined {
  const free = displays.filter((d) => !taken.has(d.id));
  const names = [meta.apiName, meta.label].filter(Boolean).map(norm);
  const isBuiltin = (d: DpiDisplay) => norm(d.name).includes('builtin');
  const apiName = (meta.apiName ?? '').toUpperCase();
  const byHardware = (d: DpiDisplay) =>
    !!d.hardwareId && apiName.includes(`(${d.hardwareId.toUpperCase()})`);
  return (
    free.find(byHardware) ??
    free.find((d) => names.includes(norm(d.name))) ??
    (meta.apiId === 'builtin' ? free.find(isBuiltin) : undefined) ??
    free.find(
      (d) =>
        !isBuiltin(d) &&
        names.some((n) => n && (norm(d.name).includes(n) || n.includes(norm(d.name)))),
    )
  );
}

/**
 * Loads DPI displays while `enabled` and exposes an apply action that
 * refreshes the list and re-anchors the popup after the OS relayouts.
 */
export function useDpiDisplays(enabled: boolean) {
  const [displays, setDisplays] = useState<DpiDisplay[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [applyingId, setApplyingId] = useState<string | null>(null);

  /** Loads displays from the backend. */
  const load = useCallback(async () => {
    try {
      setDisplays(await invoke<DpiDisplay[]>('get_dpi_displays'));
      setError(null);
    } catch (e) {
      setDisplays([]);
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    if (enabled) void load();
  }, [enabled, load]);

  /** Applies a scale to one display, refreshes, and re-anchors the popup. */
  const apply = async (id: string, percent: number) => {
    setApplyingId(id);
    setError(null);
    try {
      await invoke('set_display_dpi', { id, percent });
    } catch (e) {
      setError(String(e));
    } finally {
      setApplyingId(null);
      await load();
      for (const ms of REANCHOR_DELAY_MS)
        setTimeout(() => window.dispatchEvent(new Event('dpi-changed')), ms);
    }
  };

  return { displays, error, applyingId, apply };
}

interface DpiOptionsProps {
  enabled: boolean;
  minPercent: number;
  maxPercent: number;
  stepPercent: number;
  /** True when any display is continuous, so the step input matters. */
  showStep: boolean;
  onEnabledChange: (enabled: boolean) => void;
  onRangeChange: (min: number, max: number, step: number) => void;
}

/**
 * Beta DPI toggle for the Monitors section: checkbox with a beta chip; the
 * warning lives in its tooltip. Band inputs render hidden (`display: none`) —
 * the band is a preferences.json-only knob that affects Linux only.
 */
export function DpiOptions({
  enabled,
  minPercent,
  maxPercent,
  stepPercent,
  showStep,
  onEnabledChange,
  onRangeChange,
}: DpiOptionsProps) {
  const [minText, setMinText] = useState(String(minPercent));
  const [maxText, setMaxText] = useState(String(maxPercent));
  const [stepText, setStepText] = useState(String(stepPercent));
  const [rangeError, setRangeError] = useState<string | null>(null);

  useEffect(() => setMinText(String(minPercent)), [minPercent]);
  useEffect(() => setMaxText(String(maxPercent)), [maxPercent]);
  useEffect(() => setStepText(String(stepPercent)), [stepPercent]);

  /** Validates and commits the inputs. */
  const commit = () => {
    const result = validateDpiRange(minText, maxText, stepText);
    if ('error' in result) {
      setRangeError(result.error);
      return;
    }
    setRangeError(null);
    if (result.min !== minPercent || result.max !== maxPercent || result.step !== stepPercent)
      onRangeChange(result.min, result.max, result.step);
  };

  /** One labeled numeric input. */
  const field = (label: string, value: string, set: (v: string) => void) => (
    <label>
      {label}
      <input
        className='settings-dpi-input'
        aria-label={`${label} DPI percent`}
        inputMode='numeric'
        value={value}
        onChange={(e) => set(e.target.value)}
        onBlur={commit}
        onKeyDown={(e) => e.key === 'Enter' && commit()}
      />
    </label>
  );

  return (
    <div className='settings-dpi-options'>
      <Tooltip
        text={
          <>
            Per-display UI scaling.{' '}
            <span className='settings-dpi-beta'>Beta: changes OS scaling; apps may blur.</span>
          </>
        }>
        <label className='settings-checkbox-row'>
          <input
            type='checkbox'
            checked={enabled}
            onChange={(e) => onEnabledChange(e.target.checked)}
          />
          <span>Show DPI Settings</span>
          <span className='beta-chip'>beta</span>
        </label>
      </Tooltip>
      {/* Band inputs are hidden (display: none); edit dpiMin/Max/StepPercent in preferences.json. They only affect Linux. */}
      {enabled && (
        <>
          <div className='settings-dpi-range' style={{ display: 'none' }}>
            {field('Min %', minText, setMinText)}
            {field('Max %', maxText, setMaxText)}
            {showStep && field('Step %', stepText, setStepText)}
          </div>
          {rangeError && <div className='settings-dpi-error'>{rangeError}</div>}
        </>
      )}
    </div>
  );
}

/** Hover help for the per-display scale dropdown. */
export const DPI_TOOLTIP = 'UI size. Higher % = bigger text, less space. 100% = native.';

interface DpiScaleDropdownProps {
  display: DpiDisplay;
  minPercent: number;
  maxPercent: number;
  stepPercent: number;
  disabled: boolean;
  onApply: (id: string, percent: number) => void;
}

/** Per-display scale dropdown limited to the user band. */
export function DpiScaleDropdown({
  display: d,
  minPercent,
  maxPercent,
  stepPercent,
  disabled,
  onApply,
}: DpiScaleDropdownProps) {
  const options = dpiOptionsFor(d, minPercent, maxPercent, stepPercent);
  return (
    <Tooltip text={DPI_TOOLTIP}>
      <Dropdown
        className='monitor-dpi-scale'
        aria-label={`DPI scale for ${d.name}`}
        value={d.current == null ? '' : String(d.current)}
        disabled={disabled}
        onChange={(e) => onApply(d.id, Number(e.target.value))}>
        {d.current == null && <option value=''>Unknown</option>}
        {options.map((p) => (
          <option key={p} value={String(p)}>
            {p}%
          </option>
        ))}
      </Dropdown>
    </Tooltip>
  );
}
