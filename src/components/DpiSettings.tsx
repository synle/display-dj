import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { DpiDisplay } from '../types';
import Dropdown from './Dropdown';
import Tooltip from './Tooltip';

/** Absolute lowest DPI scale percent (mirrors backend `DPI_ABSOLUTE_MIN`). */
export const DPI_ABSOLUTE_MIN = 60;
/** Absolute highest DPI scale percent (mirrors backend `DPI_ABSOLUTE_MAX`). */
export const DPI_ABSOLUTE_MAX = 250;

interface DpiSettingsProps {
  enabled: boolean;
  minPercent: number;
  maxPercent: number;
  onEnabledChange: (enabled: boolean) => void;
  onRangeChange: (min: number, max: number) => void;
}

/**
 * Validates a min/max text pair.
 *
 * @param minText - Raw min input.
 * @param maxText - Raw max input.
 * @returns Parsed `{ min, max }` or an error message.
 */
export function validateDpiRange(
  minText: string,
  maxText: string,
): { min: number; max: number } | { error: string } {
  const isInt = (t: string) => /^\d+$/.test(t.trim());
  if (!isInt(minText) || !isInt(maxText)) return { error: 'Min and max must be whole numbers.' };
  const min = Number(minText);
  const max = Number(maxText);
  const inBand = (v: number) => v >= DPI_ABSOLUTE_MIN && v <= DPI_ABSOLUTE_MAX;
  if (!inBand(min) || !inBand(max)) {
    return { error: `Values must be between ${DPI_ABSOLUTE_MIN}% and ${DPI_ABSOLUTE_MAX}%.` };
  }
  if (min > max) return { error: 'Min must not exceed max.' };
  return { min, max };
}

/**
 * Beta DPI scaling settings: an opt-in checkbox, min/max bound inputs, and a
 * per-display scale dropdown filtered to that band. Rendered in Settings only.
 */
export default function DpiSettings({
  enabled,
  minPercent,
  maxPercent,
  onEnabledChange,
  onRangeChange,
}: DpiSettingsProps) {
  const [minText, setMinText] = useState(String(minPercent));
  const [maxText, setMaxText] = useState(String(maxPercent));
  const [rangeError, setRangeError] = useState<string | null>(null);
  const [displays, setDisplays] = useState<DpiDisplay[] | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [applyError, setApplyError] = useState<string | null>(null);
  const [applyingId, setApplyingId] = useState<string | null>(null);

  useEffect(() => setMinText(String(minPercent)), [minPercent]);
  useEffect(() => setMaxText(String(maxPercent)), [maxPercent]);

  /** Loads displays from the backend. */
  const load = async () => {
    try {
      setDisplays(await invoke<DpiDisplay[]>('get_dpi_displays'));
      setLoadError(null);
    } catch (error) {
      setDisplays([]);
      setLoadError(String(error));
    }
  };

  useEffect(() => {
    if (enabled) void load();
  }, [enabled]);

  /** Validates and commits the min/max inputs. */
  const commitRange = () => {
    const result = validateDpiRange(minText, maxText);
    if ('error' in result) {
      setRangeError(result.error);
      return;
    }
    setRangeError(null);
    if (result.min !== minPercent || result.max !== maxPercent)
      onRangeChange(result.min, result.max);
  };

  /** Applies a scale to one display and refreshes the list. */
  const apply = async (id: string, percent: number) => {
    setApplyingId(id);
    setApplyError(null);
    try {
      await invoke('set_display_dpi', { id, percent });
    } catch (error) {
      setApplyError(String(error));
    } finally {
      setApplyingId(null);
      await load();
    }
  };

  return (
    <>
      <div className='settings-section'>
        <Tooltip text='Change each display’s UI scaling (like System Settings / Display Scale).'>
          <label className='settings-checkbox-row'>
            <input
              type='checkbox'
              checked={enabled}
              onChange={(e) => onEnabledChange(e.target.checked)}
            />
            <span>Show DPI Settings (Beta)</span>
          </label>
        </Tooltip>
        {enabled && (
          <>
            <div className='settings-dpi-beta'>
              Beta: changes your OS display scaling. Apps may relayout or look blurry.
            </div>
            <div className='settings-dpi-range'>
              <label>
                Min %
                <input
                  className='settings-dpi-input'
                  aria-label='Min DPI percent'
                  inputMode='numeric'
                  value={minText}
                  onChange={(e) => setMinText(e.target.value)}
                  onBlur={commitRange}
                  onKeyDown={(e) => e.key === 'Enter' && commitRange()}
                />
              </label>
              <label>
                Max %
                <input
                  className='settings-dpi-input'
                  aria-label='Max DPI percent'
                  inputMode='numeric'
                  value={maxText}
                  onChange={(e) => setMaxText(e.target.value)}
                  onBlur={commitRange}
                  onKeyDown={(e) => e.key === 'Enter' && commitRange()}
                />
              </label>
            </div>
            {rangeError && <div className='settings-dpi-error'>{rangeError}</div>}
          </>
        )}
      </div>

      {enabled && (
        <>
          <div className='settings-divider' />
          <div className='settings-section'>
            <Tooltip text='UI scale per display. 100% = native pixels; higher = bigger text.'>
              <label className='settings-label'>DPI Settings (Beta)</label>
            </Tooltip>
            {loadError && <div className='settings-dpi-error'>{loadError}</div>}
            {applyError && <div className='settings-dpi-error'>{applyError}</div>}
            <div className='settings-monitors-list'>
              {displays?.map((d) => {
                const options = d.options.filter(
                  (p) => (p >= minPercent && p <= maxPercent) || p === d.current,
                );
                return (
                  <div key={d.id} className='settings-monitor-row'>
                    <span className='settings-audio-output-name'>{d.name}</span>
                    <Dropdown
                      className='monitor-dpi-scale'
                      aria-label={`DPI scale for ${d.name}`}
                      value={d.current == null ? '' : String(d.current)}
                      disabled={applyingId !== null}
                      onChange={(e) => void apply(d.id, Number(e.target.value))}>
                      {d.current == null && <option value=''>Unknown</option>}
                      {options.map((p) => (
                        <option
                          key={p}
                          value={String(p)}
                          disabled={p < minPercent || p > maxPercent}>
                          {p}%
                        </option>
                      ))}
                    </Dropdown>
                  </div>
                );
              })}
            </div>
          </div>
        </>
      )}
    </>
  );
}
