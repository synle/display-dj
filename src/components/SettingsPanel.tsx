import { useState, useEffect, useRef, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Preferences, MonitorMetadata, NightModeSchedule, TilingPreferences } from '../types';
import Slider from './Slider';

interface SettingsPanelProps {
  onClose: () => void;
  onPreferencesSaved: () => void;
}

/** Settings panel with two tabs: General and Tiling. Auto-saves after each change. */
export default function SettingsPanel({ onClose, onPreferencesSaved }: SettingsPanelProps) {
  const [prefs, setPrefs] = useState<Preferences | null>(null);
  const [activeTab, setActiveTab] = useState<'general' | 'tiling'>('general');
  const [editingUid, setEditingUid] = useState<string | null>(null);
  const [editLabel, setEditLabel] = useState('');
  const labelInputRef = useRef<HTMLInputElement>(null);
  const [tilingSupported, setTilingSupported] = useState(false);
  const [accessibilityTrusted, setAccessibilityTrusted] = useState(true);
  const initialLoadRef = useRef(true);
  const saveTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    invoke<Preferences>('get_preferences')
      .then((p) => {
        p.minBrightness = Math.max(5, Math.min(100, p.minBrightness));
        setPrefs(p);
        setTimeout(() => {
          initialLoadRef.current = false;
        }, 0);
      })
      .catch(console.error);
    invoke<boolean>('get_tiling_supported')
      .then(setTilingSupported)
      .catch(() => setTilingSupported(false));
    invoke<boolean>('get_accessibility_trusted')
      .then(setAccessibilityTrusted)
      .catch(() => setAccessibilityTrusted(true));
  }, []);

  /** Auto-save preferences after each change with debounce. */
  const savePreferences = useCallback(
    (prefsToSave: Preferences) => {
      if (saveTimeoutRef.current) {
        clearTimeout(saveTimeoutRef.current);
      }
      saveTimeoutRef.current = setTimeout(async () => {
        try {
          await invoke('save_preferences', { preferences: prefsToSave });
          onPreferencesSaved();
        } catch (e) {
          console.error('Failed to save preferences:', e);
        }
      }, 300);
    },
    [onPreferencesSaved],
  );

  useEffect(() => {
    return () => {
      if (saveTimeoutRef.current) {
        clearTimeout(saveTimeoutRef.current);
      }
    };
  }, []);

  if (!prefs) return null;

  const schedule = prefs.nightModeSchedule;
  const configs = [...prefs.monitorConfigs].sort(
    (a, b) => a.sortOrder - b.sortOrder || a.uid.localeCompare(b.uid),
  );

  /** Updates a top-level preference field and triggers auto-save. */
  const updateField = <K extends keyof Preferences>(key: K, value: Preferences[K]) => {
    setPrefs((prev) => {
      if (!prev) return prev;
      const next = { ...prev, [key]: value };
      if (!initialLoadRef.current) savePreferences(next);
      return next;
    });
  };

  /** Updates a field within the night mode schedule and triggers auto-save. */
  const updateSchedule = <K extends keyof NightModeSchedule>(
    key: K,
    value: NightModeSchedule[K],
  ) => {
    setPrefs((prev) => {
      if (!prev) return prev;
      const next = {
        ...prev,
        nightModeSchedule: { ...prev.nightModeSchedule, [key]: value },
      };
      if (!initialLoadRef.current) savePreferences(next);
      return next;
    });
  };

  /** Updates a field within the tiling preferences and triggers auto-save. */
  const updateTiling = <K extends keyof TilingPreferences>(key: K, value: TilingPreferences[K]) => {
    setPrefs((prev) => {
      if (!prev) return prev;
      const next = {
        ...prev,
        tiling: { ...prev.tiling, [key]: value },
      };
      if (!initialLoadRef.current) savePreferences(next);
      return next;
    });
  };

  /** Patches a single monitor's metadata and triggers auto-save. */
  const updateMonitorConfig = (uid: string, patch: Partial<MonitorMetadata>) => {
    setPrefs((prev) => {
      if (!prev) return prev;
      const next = {
        ...prev,
        monitorConfigs: prev.monitorConfigs.map((m) => (m.uid === uid ? { ...m, ...patch } : m)),
      };
      if (!initialLoadRef.current) savePreferences(next);
      return next;
    });
  };

  /** Swaps the sort order of two monitors. */
  const swapMonitorOrder = (indexA: number, indexB: number) => {
    if (!prefs) return;
    const a = configs[indexA];
    const b = configs[indexB];
    if (!a || !b) return;
    setPrefs((prev) => {
      if (!prev) return prev;
      const next = {
        ...prev,
        monitorConfigs: prev.monitorConfigs.map((m) => {
          if (m.uid === a.uid) return { ...m, sortOrder: b.sortOrder };
          if (m.uid === b.uid) return { ...m, sortOrder: a.sortOrder };
          return m;
        }),
      };
      if (!initialLoadRef.current) savePreferences(next);
      return next;
    });
  };

  /** Enters inline label edit mode for a monitor config row. */
  const startEditingLabel = (meta: MonitorMetadata) => {
    setEditingUid(meta.uid);
    setEditLabel(meta.label);
    setTimeout(() => labelInputRef.current?.focus(), 0);
  };

  /** Commits the edited label and exits edit mode. */
  const finishEditingLabel = () => {
    if (editingUid) {
      updateMonitorConfig(editingUid, { label: editLabel.trim() });
    }
    setEditingUid(null);
  };

  const tiling = prefs.tiling;
  const gridSize = Math.round(Math.sqrt(tiling?.exposeMaxWindows ?? 16));

  return (
    <div className='settings-panel'>
      <div className='settings-header'>
        <span className='settings-title'>Settings</span>
        <button className='settings-close' onClick={onClose} title='Close'>
          &times;
        </button>
      </div>

      {tilingSupported && (
        <div className='settings-tabs'>
          <button
            className={`settings-tab${activeTab === 'general' ? ' settings-tab-active' : ''}`}
            onClick={() => setActiveTab('general')}>
            General
          </button>
          <button
            className={`settings-tab${activeTab === 'tiling' ? ' settings-tab-active' : ''}`}
            onClick={() => setActiveTab('tiling')}>
            Tiling
          </button>
        </div>
      )}

      <div className='settings-body'>
        {activeTab === 'general' && (
          <>
            <div className='settings-section'>
              <label className='settings-label'>Min Brightness</label>
              <Slider
                value={prefs.minBrightness}
                min={5}
                max={100}
                onChange={(v) => updateField('minBrightness', v)}
              />
            </div>

            <div className='settings-section'>
              <label className='settings-checkbox-row'>
                <input
                  type='checkbox'
                  checked={prefs.showContrast}
                  onChange={(e) => updateField('showContrast', e.target.checked)}
                />
                <span>Show Contrast Slider</span>
              </label>
            </div>

            <div className='settings-divider' />

            <div className='settings-section'>
              <label className='settings-label'>Monitors</label>
              <div className='settings-monitors-list'>
                {configs.map((meta, index) => {
                  const displayName = meta.label || meta.apiName || meta.uid;
                  return (
                    <div
                      key={meta.uid}
                      className={`settings-monitor-row${meta.hidden ? ' settings-monitor-hidden' : ''}`}>
                      <div className='settings-monitor-reorder'>
                        <button
                          className='monitor-reorder-btn'
                          disabled={index === 0}
                          onClick={() => swapMonitorOrder(index, index - 1)}
                          title='Move up'>
                          ▲
                        </button>
                        <button
                          className='monitor-reorder-btn'
                          disabled={index === configs.length - 1}
                          onClick={() => swapMonitorOrder(index, index + 1)}
                          title='Move down'>
                          ▼
                        </button>
                      </div>
                      <div className='settings-monitor-name'>
                        {editingUid === meta.uid ? (
                          <input
                            ref={labelInputRef}
                            className='monitor-name-input'
                            value={editLabel}
                            placeholder={meta.apiName}
                            onChange={(e) => setEditLabel(e.target.value)}
                            onBlur={finishEditingLabel}
                            onKeyDown={(e) => {
                              if (e.key === 'Enter') finishEditingLabel();
                              if (e.key === 'Escape') setEditingUid(null);
                            }}
                          />
                        ) : (
                          <button className='monitor-name' onClick={() => startEditingLabel(meta)}>
                            {displayName}
                          </button>
                        )}
                      </div>
                      {meta.apiId !== 'builtin' && (
                        <button
                          className='monitor-visibility-btn'
                          onClick={() => updateMonitorConfig(meta.uid, { hidden: !meta.hidden })}
                          title={meta.hidden ? 'Show monitor' : 'Hide monitor'}>
                          {meta.hidden ? 'Show' : 'Hide'}
                        </button>
                      )}
                    </div>
                  );
                })}
              </div>
            </div>

            <div className='settings-divider' />

            <div className='settings-section'>
              <label className='settings-checkbox-row'>
                <input
                  type='checkbox'
                  checked={schedule.enabled}
                  onChange={(e) => updateSchedule('enabled', e.target.checked)}
                />
                <span>Night Mode Schedule</span>
              </label>
            </div>

            {schedule.enabled && (
              <>
                <div className='settings-section'>
                  <div className='settings-schedule-header'>
                    <label className='settings-label'>Night</label>
                    <input
                      type='time'
                      className='settings-time-input'
                      value={schedule.nightStart}
                      onChange={(e) => updateSchedule('nightStart', e.target.value)}
                    />
                  </div>
                  <Slider
                    value={schedule.nightBrightness}
                    min={5}
                    max={100}
                    onChange={(v) => updateSchedule('nightBrightness', v)}
                  />
                </div>

                <div className='settings-section'>
                  <div className='settings-schedule-header'>
                    <label className='settings-label'>Day</label>
                    <input
                      type='time'
                      className='settings-time-input'
                      value={schedule.dayStart}
                      onChange={(e) => updateSchedule('dayStart', e.target.value)}
                    />
                  </div>
                  <Slider
                    value={schedule.dayBrightness}
                    min={5}
                    max={100}
                    onChange={(v) => updateSchedule('dayBrightness', v)}
                  />
                </div>
              </>
            )}

            <div className='settings-divider' />

            <div className='settings-section'>
              <label className='settings-checkbox-row'>
                <input
                  type='checkbox'
                  checked={prefs.launchAtLogin}
                  onChange={(e) => updateField('launchAtLogin', e.target.checked)}
                />
                <span>Launch at Login</span>
              </label>
            </div>
          </>
        )}

        {activeTab === 'tiling' && (
          <>
            <div className='settings-section'>
              <label className='settings-checkbox-row'>
                <input
                  type='checkbox'
                  checked={tiling?.enabled ?? true}
                  onChange={(e) => updateTiling('enabled', e.target.checked)}
                />
                <span>Enable Window Tiling</span>
              </label>
              {tiling?.enabled && !accessibilityTrusted && (
                <div
                  style={{
                    fontSize: '11px',
                    color: '#e67700',
                    marginTop: '4px',
                    paddingLeft: '22px',
                  }}>
                  ⚠ Accessibility permission required for tiling.{' '}
                  <a
                    href='#'
                    onClick={(e) => {
                      e.preventDefault();
                      invoke('open_accessibility_settings');
                    }}
                    style={{ color: '#e67700' }}>
                    Open Accessibility Settings
                  </a>{' '}
                  |{' '}
                  <a
                    href='https://github.com/synle/display-dj#window-tiling-macos'
                    target='_blank'
                    rel='noopener noreferrer'
                    style={{ color: '#e67700' }}>
                    Learn how to enable
                  </a>
                </div>
              )}
            </div>

            {tiling?.enabled && (
              <>
                <div className='settings-divider' />

                <div className='settings-section'>
                  <label className='settings-label'>Snap Zones</label>
                  <div style={{ marginTop: '4px' }}>
                    <label className='settings-label'>Side Edge</label>
                    <Slider
                      value={tiling?.sideEdgeTrigger ?? 10}
                      min={5}
                      max={50}
                      unit='px'
                      onChange={(v) => updateTiling('sideEdgeTrigger', v)}
                    />
                  </div>
                  <div style={{ marginTop: '4px' }}>
                    <label className='settings-label'>Top Edge</label>
                    <Slider
                      value={tiling?.topEdgeTrigger ?? 10}
                      min={10}
                      max={50}
                      unit='px'
                      onChange={(v) => updateTiling('topEdgeTrigger', v)}
                    />
                  </div>
                  <div style={{ marginTop: '4px' }}>
                    <label className='settings-label'>Corner</label>
                    <Slider
                      value={tiling?.cornerTrigger ?? 50}
                      min={25}
                      max={150}
                      unit='px'
                      onChange={(v) => updateTiling('cornerTrigger', v)}
                    />
                  </div>
                </div>

                <div className='settings-divider' />

                <div className='settings-section'>
                  <label className='settings-checkbox-row'>
                    <input
                      type='checkbox'
                      checked={tiling?.exposeEnabled ?? true}
                      onChange={(e) => updateTiling('exposeEnabled', e.target.checked)}
                    />
                    <span>Enable Exposé</span>
                  </label>
                </div>

                {(tiling?.exposeEnabled ?? true) && (
                  <div className='settings-section'>
                    <label className='settings-label'>Exposé Grid Size</label>
                    <Slider
                      value={gridSize}
                      min={2}
                      max={5}
                      onChange={(v) => updateTiling('exposeMaxWindows', v * v)}
                      showValue={false}
                    />
                    <span
                      style={{
                        fontSize: '11px',
                        color: '#666',
                        marginTop: '2px',
                        display: 'block',
                      }}>
                      {gridSize} &times; {gridSize} = {tiling?.exposeMaxWindows ?? 16} windows per
                      screen
                    </span>
                  </div>
                )}
              </>
            )}
          </>
        )}
      </div>
    </div>
  );
}
