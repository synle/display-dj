import { Icon } from './Icons';
import { useState, useEffect, useRef, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  Preferences,
  AudioOutputDeviceState,
  AudioOutputState,
  MonitorMetadata,
  NightModeSchedule,
  TilingPreferences,
  WallpaperPreferences,
} from '../types';
import Dropdown from './Dropdown';
import Slider from './Slider';

interface SettingsPanelProps {
  onClose: () => void;
  onPreferencesSaved: () => void;
}

/** Settings panel with two tabs: General and Tiling. Auto-saves after each change. */
/** Minimum slideshow interval in seconds (mirrors backend `MIN_SLIDESHOW_INTERVAL_SECS`). */
const MIN_SLIDESHOW_INTERVAL_SECS = 5;
/** Minute choices for the slideshow interval (1m and 3m included for quick testing). */
const SLIDESHOW_MINUTE_OPTIONS = [0, 1, 3, 5, 10, 15, 20, 25, 30, 35, 40, 45, 50, 55];
/** Second choices for the slideshow interval. */
const SLIDESHOW_SECOND_OPTIONS = [0, 5, 10, 15, 20, 30, 45];

/**
 * Returns `options` plus `current` (sorted) when a hand-edited value is not a preset,
 * so the dropdown still shows the persisted value.
 * @param options preset values
 * @param current currently persisted value
 * @returns sorted option list containing `current`
 */
function withCurrent(options: number[], current: number): number[] {
  return options.includes(current) ? options : [...options, current].sort((a, b) => a - b);
}

/**
 * Formats an interval as e.g. `1 hour 3 minutes 30 seconds`. Zero units are
 * skipped so the text jumps to the next smaller non-zero unit.
 * @param h hours
 * @param m minutes
 * @param sec seconds
 * @returns human-readable interval string
 */
export function formatInterval(h: number, m: number, sec: number): string {
  const unit = (n: number, name: string) => (n ? `${n} ${name}${n === 1 ? '' : 's'}` : '');
  const parts = [unit(h, 'hour'), unit(m, 'minute'), unit(sec, 'second')].filter(Boolean);
  return parts.length ? parts.join(' ') : '0 seconds';
}

export default function SettingsPanel({ onClose, onPreferencesSaved }: SettingsPanelProps) {
  const [prefs, setPrefs] = useState<Preferences | null>(null);
  const [audioOutputState, setAudioOutputState] = useState<AudioOutputState | null>(null);
  const [updatingAudioOutputId, setUpdatingAudioOutputId] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<'general' | 'tiling'>('general');
  const [editingUid, setEditingUid] = useState<string | null>(null);
  const [editLabel, setEditLabel] = useState('');
  const labelInputRef = useRef<HTMLInputElement>(null);
  const [tilingSupported, setTilingSupported] = useState(false);
  const [accessibilityTrusted, setAccessibilityTrusted] = useState(true);
  const [windowsElevated, setWindowsElevated] = useState<boolean | null>(null);
  const [windowsSnapEnabled, setWindowsSnapEnabled] = useState<boolean | null>(null);
  const [platform, setPlatform] = useState<'macos' | 'windows' | 'other'>('other');
  const [loadError, setLoadError] = useState(false);
  const [saveStatus, setSaveStatus] = useState<'idle' | 'saving' | 'saved' | 'error'>('idle');
  const saveTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const pendingSaveRef = useRef<Preferences | null>(null);
  const saveInFlightRef = useRef(false);
  const mountedRef = useRef(true);

  /** Loads editable preferences and exposes failure instead of leaving a blank panel. */
  const loadPreferences = useCallback(() => {
    invoke<Preferences>('get_preferences')
      .then((p) => {
        setPrefs({ ...p, minBrightness: Math.max(5, Math.min(100, p.minBrightness)) });
      })
      .catch(() => setLoadError(true));
  }, []);

  useEffect(() => {
    loadPreferences();
    invoke<boolean>('get_tiling_supported')
      .then(setTilingSupported)
      .catch(() => setTilingSupported(false));
    invoke<boolean>('get_accessibility_trusted')
      .then(setAccessibilityTrusted)
      .catch(() => setAccessibilityTrusted(true));
    invoke<boolean | null>('get_windows_elevation_status')
      .then(setWindowsElevated)
      .catch((error) => console.error('Failed to check Windows elevation:', error));
    invoke<boolean | null>('get_windows_snap_enabled')
      .then(setWindowsSnapEnabled)
      .catch((error) => console.error('Failed to check Windows Snap:', error));
    invoke<Record<string, string>>('get_about_info')
      .then((info) => {
        if (info.os === 'macOS') setPlatform('macos');
        else if (info.os === 'Windows') setPlatform('windows');
      })
      .catch((error) => console.error('Failed to detect platform:', error));
    invoke<AudioOutputState>('get_audio_output_devices')
      .then(setAudioOutputState)
      .catch((error) => console.error('Failed to get audio output devices:', error));
  }, [loadPreferences]);

  /** Serializes full preference snapshots, coalescing queued edits to the newest state. */
  const flushSave = useCallback(
    async function drainSaveQueue() {
      if (saveInFlightRef.current || !pendingSaveRef.current) return;
      saveInFlightRef.current = true;
      const snapshot = pendingSaveRef.current;
      pendingSaveRef.current = null;
      if (mountedRef.current) setSaveStatus('saving');
      try {
        await invoke('save_preferences', { preferences: snapshot });
        onPreferencesSaved();
        if (mountedRef.current) setSaveStatus(pendingSaveRef.current ? 'saving' : 'saved');
      } catch {
        if (!pendingSaveRef.current) pendingSaveRef.current = snapshot;
        if (mountedRef.current) setSaveStatus('error');
        saveInFlightRef.current = false;
        return;
      }
      saveInFlightRef.current = false;
      if (pendingSaveRef.current) void drainSaveQueue();
    },
    [onPreferencesSaved],
  );

  /** Debounces edits while retaining the latest complete snapshot for close/unmount flush. */
  const savePreferences = useCallback(
    (prefsToSave: Preferences) => {
      pendingSaveRef.current = prefsToSave;
      setSaveStatus('saving');
      if (saveTimeoutRef.current) clearTimeout(saveTimeoutRef.current);
      saveTimeoutRef.current = setTimeout(() => void flushSave(), 100);
    },
    [flushSave],
  );

  useEffect(() => {
    return () => {
      mountedRef.current = false;
      if (saveTimeoutRef.current) clearTimeout(saveTimeoutRef.current);
      void flushSave();
    };
  }, [flushSave]);

  if (!prefs) {
    return (
      <div className='settings-panel'>
        <div className='settings-header'>
          <span className='settings-title'>Settings</span>
          <button className='settings-close' onClick={onClose} title='Close'>
            &times;
          </button>
        </div>
        <div className='status-message' role={loadError ? 'alert' : 'status'} aria-live='polite'>
          {loadError ? 'Could not load settings.' : 'Loading settings...'}
          {loadError && (
            <button
              onClick={() => {
                setLoadError(false);
                loadPreferences();
              }}>
              Retry
            </button>
          )}
        </div>
      </div>
    );
  }

  const schedule = prefs.nightModeSchedule;
  const configs = [...prefs.monitorConfigs].toSorted(
    (a, b) => a.sortOrder - b.sortOrder || a.uid.localeCompare(b.uid),
  );

  /** Updates a top-level preference field and triggers auto-save. */
  const updateField = <K extends keyof Preferences>(key: K, value: Preferences[K]) => {
    setPrefs((prev) => {
      if (!prev) return prev;
      const next = { ...prev, [key]: value };
      savePreferences(next);
      return next;
    });
  };

  const intervalTotalMinutes = prefs?.wallpaper?.slideshowIntervalMinutes ?? 30;
  const intervalHours = Math.floor(intervalTotalMinutes / 60);
  const intervalMinutes = intervalTotalMinutes % 60;
  const intervalSeconds = prefs?.wallpaper?.slideshowIntervalSeconds ?? 0;

  /**
   * Persists a new slideshow interval, enforcing the minimum total of
   * `MIN_SLIDESHOW_INTERVAL_SECS` (bumps seconds when everything is zero).
   * @param h hours
   * @param m minutes
   * @param sec seconds
   */
  const updateSlideshowInterval = (h: number, m: number, sec: number) => {
    const totalMinutes = h * 60 + m;
    const seconds = totalMinutes === 0 ? Math.max(MIN_SLIDESHOW_INTERVAL_SECS, sec) : sec;
    setPrefs((prev) => {
      if (!prev) return prev;
      const next = {
        ...prev,
        wallpaper: {
          ...prev.wallpaper,
          slideshowIntervalMinutes: totalMinutes,
          slideshowIntervalSeconds: seconds,
        },
      };
      savePreferences(next);
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
      savePreferences(next);
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
      savePreferences(next);
      return next;
    });
  };

  /** Updates a field within the wallpaper preferences and triggers auto-save. */
  const updateWallpaper = <K extends keyof WallpaperPreferences>(
    key: K,
    value: WallpaperPreferences[K],
  ) => {
    setPrefs((prev) => {
      if (!prev) return prev;
      const next = {
        ...prev,
        wallpaper: { ...prev.wallpaper, [key]: value },
      };
      savePreferences(next);
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
      savePreferences(next);
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
      savePreferences(next);
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

  /** Moves one speaker by one row and persists the complete device order. */
  const moveAudioOutput = async (index: number, direction: 'up' | 'down') => {
    if (!audioOutputState) return;
    const swapIndex = direction === 'up' ? index - 1 : index + 1;
    if (swapIndex < 0 || swapIndex >= audioOutputState.devices.length) return;
    const devices = [...audioOutputState.devices];
    [devices[index], devices[swapIndex]] = [devices[swapIndex], devices[index]];
    setAudioOutputState({ ...audioOutputState, devices });
    try {
      const outputState = await invoke<AudioOutputState>('save_audio_output_order', {
        orderedIds: devices.map((device) => device.id),
      });
      setAudioOutputState(outputState);
      onPreferencesSaved();
    } catch (error) {
      setAudioOutputState(audioOutputState);
      console.error('Failed to reorder audio output devices:', error);
    }
  };

  /** Persists one speaker's enabled, disabled, or hidden state. */
  const updateAudioOutputState = async (id: string, deviceState: AudioOutputDeviceState) => {
    if (!audioOutputState) return;
    const previousState = audioOutputState;
    setUpdatingAudioOutputId(id);
    setAudioOutputState({
      ...audioOutputState,
      devices: audioOutputState.devices.map((device) =>
        device.id === id ? { ...device, state: deviceState } : device,
      ),
    });
    try {
      const outputState = await invoke<AudioOutputState>('set_audio_output_device_state', {
        id,
        deviceState,
      });
      setAudioOutputState(outputState);
      onPreferencesSaved();
    } catch (error) {
      setAudioOutputState(previousState);
      console.error('Failed to update audio output device state:', error);
    } finally {
      setUpdatingAudioOutputId(null);
    }
  };

  const tiling = prefs.tiling;
  const exposeCols = tiling?.exposeColumns ?? 2;
  const exposeRows = tiling?.exposeRows ?? 3;

  return (
    <div className='settings-panel'>
      <div className='settings-header'>
        <span className='settings-title'>Settings</span>
        <span
          className={`save-status save-status-${saveStatus}`}
          role={saveStatus === 'error' ? 'alert' : 'status'}
          aria-live='polite'>
          {saveStatus === 'saving'
            ? 'Saving...'
            : saveStatus === 'saved'
              ? 'Saved'
              : saveStatus === 'error'
                ? 'Save failed'
                : ''}
          {saveStatus === 'error' && <button onClick={() => void flushSave()}>Retry</button>}
        </span>
        <button
          className='settings-close'
          onClick={() => {
            if (saveTimeoutRef.current) clearTimeout(saveTimeoutRef.current);
            void flushSave();
            onClose();
          }}
          title='Close'>
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
                label='Minimum brightness'
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
                          <Icon name='chevronUp' size={12} />
                        </button>
                        <button
                          className='monitor-reorder-btn'
                          disabled={index === configs.length - 1}
                          onClick={() => swapMonitorOrder(index, index + 1)}
                          title='Move down'>
                          <Icon name='chevronDown' size={12} />
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
                        <>
                          <Dropdown
                            className='monitor-brightness-mode'
                            value={meta.brightnessMode || 'auto'}
                            onChange={(e) =>
                              updateMonitorConfig(meta.uid, {
                                brightnessMode: e.target.value as
                                  | 'auto'
                                  | 'ddc'
                                  | 'gamma'
                                  | 'overlay',
                              })
                            }
                            title={
                              'Brightness control strategy.\n' +
                              'auto: DDC -> gamma -> soft-overlay fallback.\n' +
                              'ddc: DDC/CI only (no overlay).\n' +
                              'gamma: SetDeviceGammaRamp only (no overlay).\n' +
                              'overlay: software dimming window (works on any monitor).'
                            }>
                            <option value='auto'>Auto</option>
                            <option value='ddc'>DDC</option>
                            <option value='gamma'>Gamma</option>
                            <option value='overlay'>Overlay</option>
                          </Dropdown>
                          <button
                            className='monitor-visibility-btn'
                            onClick={() => updateMonitorConfig(meta.uid, { hidden: !meta.hidden })}
                            title={meta.hidden ? 'Show monitor' : 'Hide monitor'}>
                            {meta.hidden ? 'Show' : 'Hide'}
                          </button>
                        </>
                      )}
                    </div>
                  );
                })}
              </div>
            </div>

            <div className='settings-divider' />

            <div className='settings-section'>
              <label className='settings-label'>Speakers</label>
              <div className='settings-monitors-list'>
                {audioOutputState?.devices.map((device, index) => (
                  <div
                    key={device.id}
                    className={`settings-monitor-row${device.state === 'hidden' ? ' settings-monitor-hidden' : ''}`}>
                    <div className='settings-monitor-reorder'>
                      <button
                        className='monitor-reorder-btn'
                        disabled={index === 0}
                        onClick={() => moveAudioOutput(index, 'up')}
                        title={`Move ${device.name} up`}>
                        <Icon name='chevronUp' size={12} />
                      </button>
                      <button
                        className='monitor-reorder-btn'
                        disabled={index === audioOutputState.devices.length - 1}
                        onClick={() => moveAudioOutput(index, 'down')}
                        title={`Move ${device.name} down`}>
                        <Icon name='chevronDown' size={12} />
                      </button>
                    </div>
                    <span className='settings-audio-output-name'>{device.name}</span>
                    <Dropdown
                      className='audio-output-state'
                      aria-label={`State for ${device.name}`}
                      value={device.state}
                      disabled={updatingAudioOutputId !== null}
                      onChange={(event) =>
                        updateAudioOutputState(
                          device.id,
                          event.target.value as AudioOutputDeviceState,
                        )
                      }>
                      <option value='enabled'>Enabled</option>
                      <option value='disabled'>Disabled</option>
                      <option value='hidden'>Hidden</option>
                    </Dropdown>
                  </div>
                ))}
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
                    label='Night brightness'
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
                    label='Day brightness'
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
              <label className='settings-label'>Wallpaper Fit</label>
              <Dropdown
                className='settings-dropdown'
                value={prefs.wallpaper?.fit ?? 'fill'}
                onChange={(e) => updateWallpaper('fit', e.target.value)}>
                <option value='fill'>Fill Screen</option>
                <option value='fit'>Fit to Screen</option>
                <option value='stretch'>Stretch</option>
                <option value='center'>Center</option>
                <option value='tile'>Tile</option>
              </Dropdown>
            </div>

            <div className='settings-divider' />

            <div className='settings-section'>
              <label className='settings-checkbox-row'>
                <input
                  type='checkbox'
                  checked={prefs.wallpaper?.slideshowEnabled ?? false}
                  onChange={(e) => updateWallpaper('slideshowEnabled', e.target.checked)}
                />
                <span>Enable Wallpaper Slideshow</span>
              </label>
            </div>

            {prefs.wallpaper?.slideshowEnabled && (
              <>
                <div className='settings-section'>
                  <label className='settings-label'>Slideshow Folder</label>
                  <input
                    type='text'
                    value={prefs.wallpaper?.slideshowFolder ?? ''}
                    placeholder='/path/to/wallpapers'
                    onChange={(e) => updateWallpaper('slideshowFolder', e.target.value || null)}
                    style={{ marginTop: '4px', width: '100%', boxSizing: 'border-box' }}
                  />
                </div>

                <div className='settings-section'>
                  <label className='settings-label'>Interval</label>
                  <div style={{ display: 'flex', gap: '8px', marginTop: '4px' }}>
                    <div style={{ flex: 1 }}>
                      <label className='settings-label'>Hours</label>
                      <Dropdown
                        className='settings-dropdown'
                        value={intervalHours}
                        onChange={(e) =>
                          updateSlideshowInterval(
                            parseInt(e.target.value),
                            intervalMinutes,
                            intervalSeconds,
                          )
                        }>
                        {Array.from({ length: 25 }, (_, i) => (
                          <option key={i} value={i}>
                            {i}h
                          </option>
                        ))}
                      </Dropdown>
                    </div>
                    <div style={{ flex: 1 }}>
                      <label className='settings-label'>Minutes</label>
                      <Dropdown
                        className='settings-dropdown'
                        value={intervalMinutes}
                        onChange={(e) =>
                          updateSlideshowInterval(
                            intervalHours,
                            parseInt(e.target.value),
                            intervalSeconds,
                          )
                        }>
                        {withCurrent(SLIDESHOW_MINUTE_OPTIONS, intervalMinutes).map((m) => (
                          <option key={m} value={m}>
                            {m}m
                          </option>
                        ))}
                      </Dropdown>
                    </div>
                    <div style={{ flex: 1 }}>
                      <label className='settings-label'>Seconds</label>
                      <Dropdown
                        className='settings-dropdown'
                        value={intervalSeconds}
                        onChange={(e) =>
                          updateSlideshowInterval(
                            intervalHours,
                            intervalMinutes,
                            parseInt(e.target.value),
                          )
                        }>
                        {withCurrent(SLIDESHOW_SECOND_OPTIONS, intervalSeconds).map((sec) => (
                          <option key={sec} value={sec}>
                            {sec}s
                          </option>
                        ))}
                      </Dropdown>
                    </div>
                  </div>
                  <span
                    style={{
                      fontSize: '11px',
                      color: '#666',
                      marginTop: '2px',
                      display: 'block',
                    }}>
                    Changes every {formatInterval(intervalHours, intervalMinutes, intervalSeconds)}
                  </span>
                </div>

                <div className='settings-section'>
                  <label className='settings-label'>Slideshow Order</label>
                  <Dropdown
                    className='settings-dropdown'
                    value={prefs.wallpaper?.slideshowOrder ?? 'forward'}
                    onChange={(e) => updateWallpaper('slideshowOrder', e.target.value)}>
                    <option value='forward'>File name (A → Z)</option>
                    <option value='backward'>File name (Z → A)</option>
                    <option value='oldest'>Date modified (oldest first)</option>
                    <option value='newest'>Date modified (newest first)</option>
                    <option value='random'>Shuffle</option>
                  </Dropdown>
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
              <div className='settings-status-row'>
                <label className='settings-checkbox-row'>
                  <input
                    type='checkbox'
                    checked={tiling?.enabled ?? true}
                    onChange={(e) => updateTiling('enabled', e.target.checked)}
                  />
                  <span>Enable Window Tiling</span>
                </label>
              </div>
              {platform === 'macos' && !accessibilityTrusted && (
                <button
                  className='settings-status-error'
                  onClick={() => invoke('open_accessibility_settings')}>
                  Accessibility permission required. Open Settings
                </button>
              )}
              {platform === 'windows' && windowsElevated === false && (
                <div className='settings-status-error' role='status'>
                  Not running as administrator; elevated windows cannot be resized.
                </div>
              )}
            </div>

            {tiling?.enabled && (
              <>
                <div className='settings-divider' />

                <div className='settings-section'>
                  <div className='settings-status-row'>
                    <label className='settings-checkbox-row'>
                      <input
                        type='checkbox'
                        checked={tiling.tileSnapEnabled}
                        onChange={(e) => updateTiling('tileSnapEnabled', e.target.checked)}
                      />
                      <span>Enable Tile Snap (drag to edge)</span>
                    </label>
                  </div>
                  {platform === 'windows' && windowsSnapEnabled === true && (
                    <button
                      className='settings-status-error'
                      onClick={() => invoke('open_windows_multitasking_settings')}>
                      Windows Snap may interfere. Open Multitasking Settings
                    </button>
                  )}
                </div>

                {tiling.tileSnapEnabled && (
                  <div className='settings-section'>
                    <label className='settings-label'>Snap Zones</label>
                    <div style={{ marginTop: '4px' }}>
                      <label className='settings-label'>Side Edge</label>
                      <Slider
                        label='Side edge trigger'
                        value={tiling?.sideEdgeTrigger ?? 18}
                        min={5}
                        max={50}
                        unit='px'
                        onChange={(v) => updateTiling('sideEdgeTrigger', v)}
                      />
                    </div>
                    <div style={{ marginTop: '4px' }}>
                      <label className='settings-label'>Top Edge</label>
                      <Slider
                        label='Top edge trigger'
                        value={tiling?.topEdgeTrigger ?? 18}
                        min={10}
                        max={50}
                        unit='px'
                        onChange={(v) => updateTiling('topEdgeTrigger', v)}
                      />
                    </div>
                    <div style={{ marginTop: '4px' }}>
                      <label className='settings-label'>Corner</label>
                      <Slider
                        label='Corner trigger'
                        value={tiling?.cornerTrigger ?? 30}
                        min={25}
                        max={150}
                        unit='px'
                        onChange={(v) => updateTiling('cornerTrigger', v)}
                      />
                    </div>

                    {/* Per-zone visibility toggles. Each checkbox disables both
                        the drawn drop-zone indicator AND the cursor hit-test
                        for that zone, so a disabled zone is truly inert.
                        Grouped (edges / corners / bottom row) so the dialog
                        doesn't read as nine flat checkboxes. */}
                    <div style={{ marginTop: '12px' }}>
                      <label className='settings-label'>Zone Visibility</label>

                      <div
                        style={{
                          marginTop: '6px',
                          fontSize: '11px',
                          opacity: 0.7,
                          textTransform: 'uppercase',
                          letterSpacing: '0.04em',
                        }}>
                        Edges
                      </div>
                      <label className='settings-checkbox-row'>
                        <input
                          type='checkbox'
                          checked={tiling?.snapTopEdgeEnabled ?? true}
                          onChange={(e) => updateTiling('snapTopEdgeEnabled', e.target.checked)}
                        />
                        <span>Top edge (maximize)</span>
                      </label>
                      <label className='settings-checkbox-row'>
                        <input
                          type='checkbox'
                          checked={tiling?.snapLeftEdgeEnabled ?? true}
                          onChange={(e) => updateTiling('snapLeftEdgeEnabled', e.target.checked)}
                        />
                        <span>Left edge (left half)</span>
                      </label>
                      <label className='settings-checkbox-row'>
                        <input
                          type='checkbox'
                          checked={tiling?.snapRightEdgeEnabled ?? true}
                          onChange={(e) => updateTiling('snapRightEdgeEnabled', e.target.checked)}
                        />
                        <span>Right edge (right half)</span>
                      </label>

                      <div
                        style={{
                          marginTop: '8px',
                          fontSize: '11px',
                          opacity: 0.7,
                          textTransform: 'uppercase',
                          letterSpacing: '0.04em',
                        }}>
                        Corners
                      </div>
                      <label className='settings-checkbox-row'>
                        <input
                          type='checkbox'
                          checked={tiling?.snapTopLeftCornerEnabled ?? true}
                          onChange={(e) =>
                            updateTiling('snapTopLeftCornerEnabled', e.target.checked)
                          }
                        />
                        <span>Top-left corner</span>
                      </label>
                      <label className='settings-checkbox-row'>
                        <input
                          type='checkbox'
                          checked={tiling?.snapTopRightCornerEnabled ?? true}
                          onChange={(e) =>
                            updateTiling('snapTopRightCornerEnabled', e.target.checked)
                          }
                        />
                        <span>Top-right corner</span>
                      </label>
                      <label className='settings-checkbox-row'>
                        <input
                          type='checkbox'
                          checked={tiling?.snapBottomLeftCornerEnabled ?? true}
                          onChange={(e) =>
                            updateTiling('snapBottomLeftCornerEnabled', e.target.checked)
                          }
                        />
                        <span>Bottom-left corner</span>
                      </label>
                      <label className='settings-checkbox-row'>
                        <input
                          type='checkbox'
                          checked={tiling?.snapBottomRightCornerEnabled ?? true}
                          onChange={(e) =>
                            updateTiling('snapBottomRightCornerEnabled', e.target.checked)
                          }
                        />
                        <span>Bottom-right corner</span>
                      </label>

                      <div
                        style={{
                          marginTop: '8px',
                          fontSize: '11px',
                          opacity: 0.7,
                          textTransform: 'uppercase',
                          letterSpacing: '0.04em',
                        }}>
                        Bottom row
                      </div>
                      <label className='settings-checkbox-row'>
                        <input
                          type='checkbox'
                          checked={tiling?.snapBottomThirdsEnabled ?? true}
                          onChange={(e) =>
                            updateTiling('snapBottomThirdsEnabled', e.target.checked)
                          }
                        />
                        <span>1/3 splits (left / center / right thirds)</span>
                      </label>
                      <label className='settings-checkbox-row'>
                        <input
                          type='checkbox'
                          checked={tiling?.snapBottomTwoThirdsEnabled ?? true}
                          onChange={(e) =>
                            updateTiling('snapBottomTwoThirdsEnabled', e.target.checked)
                          }
                        />
                        <span>2/3 splits (left-2/3 / right-2/3, 2× width)</span>
                      </label>
                    </div>
                  </div>
                )}

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
                    <div style={{ marginTop: '4px' }}>
                      <label className='settings-label'>Columns</label>
                      <Slider
                        label='Exposé columns'
                        value={exposeCols}
                        min={1}
                        max={5}
                        unit=''
                        onChange={(v) => updateTiling('exposeColumns', v)}
                      />
                    </div>
                    <div style={{ marginTop: '4px' }}>
                      <label className='settings-label'>Rows</label>
                      <Slider
                        label='Exposé rows'
                        value={exposeRows}
                        min={1}
                        max={5}
                        unit=''
                        onChange={(v) => updateTiling('exposeRows', v)}
                      />
                    </div>
                    <span
                      style={{
                        fontSize: '11px',
                        color: '#666',
                        marginTop: '2px',
                        display: 'block',
                      }}>
                      {exposeCols} &times; {exposeRows} = {exposeCols * exposeRows} windows per
                      screen
                    </span>
                    <div style={{ marginTop: '8px' }}>
                      <label className='settings-label'>Layout Strategy</label>
                      <Dropdown
                        className='settings-dropdown'
                        value={tiling?.exposeLayoutStrategy ?? 'spread'}
                        onChange={(e) => updateTiling('exposeLayoutStrategy', e.target.value)}>
                        <option value='spread'>Spread (distribute evenly across displays)</option>
                        <option value='fill'>Fill (pack each display before using next)</option>
                      </Dropdown>
                    </div>
                    <div style={{ marginTop: '8px' }}>
                      <label className='settings-label'>Min Cell Width</label>
                      <Slider
                        label='Minimum cell width'
                        value={tiling?.exposeMinWidth ?? 400}
                        min={100}
                        max={800}
                        unit='px'
                        onChange={(v) => updateTiling('exposeMinWidth', v)}
                      />
                    </div>
                    <div style={{ marginTop: '4px' }}>
                      <label className='settings-label'>Min Cell Height</label>
                      <Slider
                        label='Minimum cell height'
                        value={tiling?.exposeMinHeight ?? 300}
                        min={100}
                        max={600}
                        unit='px'
                        onChange={(v) => updateTiling('exposeMinHeight', v)}
                      />
                    </div>
                    <span
                      style={{
                        fontSize: '11px',
                        color: '#666',
                        marginTop: '2px',
                        display: 'block',
                      }}>
                      Minimum grid cell size in logical pixels (scaled by DPI on Windows)
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
