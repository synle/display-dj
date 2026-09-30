import { useState, useEffect, useRef, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  Preferences,
  AudioOutputDeviceState,
  AudioOutputState,
  DpiDisplay,
  Monitor,
  MonitorMetadata,
  NightModeSchedule,
  TilingPreferences,
  WallpaperPreferences,
} from '../types';
import Dropdown from './Dropdown';
import Tooltip from './Tooltip';
import Slider from './Slider';
import { Icon } from './Icons';
import {
  applyProfileSettings,
  parseProfileSettings,
  speakerKind,
  type ProfileSettings,
} from '../profileSettings';
import { DpiOptions, DpiScaleDropdown, pairDpiDisplays, useDpiDisplays } from './DpiSettings';

interface SettingsPanelProps {
  onClose: () => void;
  onPreferencesSaved: () => void;
}

/** Settings panel with tabs: Monitors & Speakers, System, and Tiling. Auto-saves after each change. */
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
  const [activeTab, setActiveTab] = useState<'general' | 'system' | 'tiling'>('general');
  /** Profile index shown in the Profiles card editor; `null` = first in display order. */
  const [selectedProfileIdx, setSelectedProfileIdx] = useState<number | null>(null);
  const [editingUid, setEditingUid] = useState<string | null>(null);
  const [editLabel, setEditLabel] = useState('');
  const labelInputRef = useRef<HTMLInputElement>(null);
  const [tilingSupported, setTilingSupported] = useState(false);
  const [accessibilityTrusted, setAccessibilityTrusted] = useState(true);
  const [windowsElevated, setWindowsElevated] = useState<boolean | null>(null);
  const [windowsSnapEnabled, setWindowsSnapEnabled] = useState<boolean | null>(null);
  const [platform, setPlatform] = useState<'macos' | 'windows' | 'other'>('other');
  const [loadError, setLoadError] = useState(false);
  const saveTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const pendingSaveRef = useRef<Preferences | null>(null);
  const saveInFlightRef = useRef(false);

  /** Loads editable preferences and exposes failure instead of leaving a blank panel. */
  /** uids of connected monitors; `null` until loaded (then all configs show). */
  const [liveUids, setLiveUids] = useState<Set<string> | null>(null);

  /**
   * Loads monitors first (the backend dedupes moved/stale monitor configs
   * there), then preferences, so Settings lists the same displays as the main screen.
   */
  const loadPreferences = useCallback(() => {
    invoke<Monitor[]>('get_monitors')
      .then((ms) =>
        setLiveUids(Array.isArray(ms) && ms.length > 0 ? new Set(ms.map((m) => m.uid)) : null),
      )
      .catch(() => setLiveUids(null))
      .then(() => invoke<Preferences>('get_preferences'))
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
      try {
        await invoke('save_preferences', { preferences: snapshot });
        onPreferencesSaved();
      } catch {
        if (!pendingSaveRef.current) pendingSaveRef.current = snapshot;
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
      if (saveTimeoutRef.current) clearTimeout(saveTimeoutRef.current);
      saveTimeoutRef.current = setTimeout(() => void flushSave(), 100);
    },
    [flushSave],
  );

  useEffect(() => {
    return () => {
      if (saveTimeoutRef.current) clearTimeout(saveTimeoutRef.current);
      void flushSave();
    };
  }, [flushSave]);

  const dpiEnabled = prefs?.showDpiSettings ?? false;
  const dpi = useDpiDisplays(dpiEnabled);

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
  const configs = prefs.monitorConfigs
    .filter((m) => !liveUids || liveUids.has(m.uid))
    .toSorted((a, b) => a.sortOrder - b.sortOrder || a.uid.localeCompare(b.uid));
  const dpiMin = prefs.dpiMinPercent ?? 60;
  const dpiMax = prefs.dpiMaxPercent ?? 250;
  const dpiStep = prefs.dpiStepPercent ?? 5;
  // Pair each monitor row with its DPI display; leftovers render as DPI-only rows.
  const { matches: dpiMatches, unmatched: dpiUnmatched } = pairDpiDisplays(configs, dpi.displays);

  /** DPI scale dropdown for one display, bound to the current band. */
  const renderDpiDropdown = (d: DpiDisplay) => (
    <DpiScaleDropdown
      display={d}
      minPercent={dpiMin}
      maxPercent={dpiMax}
      stepPercent={dpiStep}
      disabled={dpi.applyingId !== null}
      onApply={(id, percent) => void dpi.apply(id, percent)}
    />
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

  /** Rewrites the theme / brightness / volume commands of profile `idx` and triggers auto-save. */
  const updateProfileSettings = (idx: number, settings: ProfileSettings) => {
    setPrefs((prev) => {
      if (!prev || !prev.profiles[idx]) return prev;
      const profiles = prev.profiles.map((p, i) =>
        i === idx ? applyProfileSettings(p, settings) : p,
      );
      const next = { ...prev, profiles };
      savePreferences(next);
      return next;
    });
  };

  // Day profile, then night profile, then every other profile (e.g. Presentation); deduped.
  const editableProfileIndexes = [
    ...new Set([
      ...[schedule.dayProfile || 'Daylight', schedule.nightProfile || 'Focus']
        .map((name) => prefs.profiles.findIndex((p) => p.name.toLowerCase() === name.toLowerCase()))
        .filter((idx) => idx >= 0),
      ...prefs.profiles.map((_, i) => i),
    ]),
  ];

  // Selected profile falls back to the first listed one (day profile) when unset or stale.
  const activeProfileIdx =
    selectedProfileIdx !== null && editableProfileIndexes.includes(selectedProfileIdx)
      ? selectedProfileIdx
      : editableProfileIndexes[0];

  /**
   * Renders one schedule row: start time plus the profile applied at that time.
   * @param label - "Night" or "Day"
   * @param timeKey - schedule field holding the "HH:MM" start
   * @param profileKey - schedule field holding the profile name
   */
  const renderScheduleRow = (
    label: string,
    timeKey: 'nightStart' | 'dayStart',
    profileKey: 'nightProfile' | 'dayProfile',
  ) => (
    <div className='settings-section'>
      <div className='settings-schedule-header'>
        <Tooltip text={`${label} start time and the profile it applies.`}>
          <label className='settings-label'>{label}</label>
        </Tooltip>
        <input
          type='time'
          className='settings-time-input'
          aria-label={`${label} start`}
          value={schedule[timeKey]}
          onChange={(e) => updateSchedule(timeKey, e.target.value)}
        />
      </div>
      <Dropdown
        className='settings-dropdown'
        aria-label={`${label} profile`}
        value={schedule[profileKey]}
        onChange={(e) => updateSchedule(profileKey, e.target.value)}>
        {prefs.profiles.map((p, i) => (
          <option key={i} value={p.name}>
            {p.name || `Profile ${i + 1}`}
          </option>
        ))}
      </Dropdown>
    </div>
  );

  /**
   * Renders the theme / brightness / volume editor for one profile.
   * Missing brightness or volume shows 100% / 50% until the user edits it.
   * @param idx - index into `prefs.profiles`
   */
  const renderProfileEditor = (idx: number) => {
    const profile = prefs.profiles[idx];
    const current = parseProfileSettings(profile);
    return (
      <div key={idx} className='settings-profile-editor'>
        <Dropdown
          className='settings-dropdown'
          aria-label={`${profile.name} theme`}
          value={current.theme ?? ''}
          onChange={(e) =>
            updateProfileSettings(idx, {
              ...current,
              theme: (e.target.value || null) as ProfileSettings['theme'],
            })
          }>
          <option value=''>Theme: Keep current</option>
          <option value='dark'>Theme: Dark</option>
          <option value='light'>Theme: Light</option>
        </Dropdown>
        <Slider
          label={`${profile.name} brightness`}
          value={current.brightness ?? 100}
          min={prefs.minBrightness}
          max={100}
          onChange={(v) => updateProfileSettings(idx, { ...current, brightness: v })}
        />
        <Slider
          label={`${profile.name} volume`}
          value={current.volume ?? 50}
          min={0}
          max={100}
          onChange={(v) => updateProfileSettings(idx, { ...current, volume: v })}
        />
      </div>
    );
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

      <div className='settings-tabs'>
        <button
          className={`settings-tab${activeTab === 'general' ? ' settings-tab-active' : ''}`}
          onClick={() => setActiveTab('general')}>
          Monitors &amp; Speakers
        </button>
        <button
          className={`settings-tab${activeTab === 'system' ? ' settings-tab-active' : ''}`}
          onClick={() => setActiveTab('system')}>
          System
        </button>
        {tilingSupported && (
          <button
            className={`settings-tab${activeTab === 'tiling' ? ' settings-tab-active' : ''}`}
            onClick={() => setActiveTab('tiling')}>
            Tiling
          </button>
        )}
      </div>

      <div className='settings-body'>
        {activeTab === 'general' && (
          <>
            <div className='settings-card settings-card-monitors'>
              <Tooltip text='Rename, scale, dim, or hide monitors.'>
                <div className='settings-card-title'>Monitors</div>
              </Tooltip>
              <Tooltip text='Brightness floor so screens never go black.'>
                <div className='settings-subheader'>Min Brightness</div>
              </Tooltip>
              <Slider
                label='Minimum brightness'
                value={prefs.minBrightness}
                min={5}
                max={100}
                onChange={(v) => updateField('minBrightness', v)}
              />
              <div className='settings-subheader'>Options</div>
              <Tooltip text='Contrast slider for external monitors.'>
                <label className='settings-checkbox-row'>
                  <input
                    type='checkbox'
                    checked={prefs.showContrast}
                    onChange={(e) => updateField('showContrast', e.target.checked)}
                  />
                  <span>Show Contrast Slider</span>
                </label>
              </Tooltip>
              <DpiOptions
                enabled={dpiEnabled}
                minPercent={dpiMin}
                maxPercent={dpiMax}
                stepPercent={dpiStep}
                showStep={dpi.displays.some((d) => d.continuous)}
                onEnabledChange={(v) => updateField('showDpiSettings', v)}
                onRangeChange={(min, max, step) =>
                  setPrefs((prev) => {
                    if (!prev) return prev;
                    const next = {
                      ...prev,
                      dpiMinPercent: min,
                      dpiMaxPercent: max,
                      dpiStepPercent: step,
                    };
                    savePreferences(next);
                    return next;
                  })
                }
              />
              {dpiEnabled && dpi.error && <div className='settings-dpi-error'>{dpi.error}</div>}
              <div className='settings-subheader'>Displays</div>
              <div className='settings-monitors-list'>
                {configs.map((meta) => {
                  const displayName = meta.label || meta.apiName || meta.uid;
                  const dpiDisplay = dpiEnabled ? dpiMatches.get(meta.uid) : undefined;
                  return (
                    <div
                      key={meta.uid}
                      className={`settings-monitor-row${meta.hidden ? ' settings-monitor-hidden' : ''}`}>
                      <div className='settings-monitor-name'>
                        <span className='settings-device-icon'>
                          <Icon name={meta.apiId === 'builtin' ? 'laptop' : 'monitor'} size={16} />
                        </span>
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
                      {dpiDisplay && renderDpiDropdown(dpiDisplay)}
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
                {dpiEnabled &&
                  dpiUnmatched.map((d) => (
                    <div key={`dpi-${d.id}`} className='settings-monitor-row'>
                      <span className='settings-audio-output-name'>{d.name}</span>
                      {renderDpiDropdown(d)}
                    </div>
                  ))}
              </div>
            </div>

            <div className='settings-card settings-card-speakers'>
              <Tooltip text='Rename, enable, or hide speakers.'>
                <div className='settings-card-title'>Speakers</div>
              </Tooltip>
              <div className='settings-monitors-list'>
                {audioOutputState?.devices.map((device) => (
                  <div
                    key={device.id}
                    className={`settings-monitor-row${device.state === 'hidden' ? ' settings-monitor-hidden' : ''}`}>
                    <span className='settings-device-icon'>
                      <Icon
                        name={
                          speakerKind(device.originalName) === 'headphones'
                            ? 'headphones'
                            : 'speakerDevice'
                        }
                        size={16}
                      />
                    </span>
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

              {platform === 'windows' && (
                <div className='settings-section'>
                  <Tooltip text='Even out quiet and loud audio. Windows only.'>
                    <label className='settings-checkbox-row'>
                      <input
                        type='checkbox'
                        checked={prefs.loudnessEqualizationPreferred ?? true}
                        onChange={(e) =>
                          updateField('loudnessEqualizationPreferred', e.target.checked)
                        }
                      />
                      <span>Prefer Loudness Equalization</span>
                      <span className='beta-chip platform-chip'>Windows</span>
                    </label>
                  </Tooltip>
                </div>
              )}
            </div>
          </>
        )}

        {activeTab === 'system' && (
          <>
            <div className='settings-card settings-card-startup'>
              <div className='settings-card-title'>Startup</div>
              <div className='settings-section'>
                <Tooltip text='Start on sign in.'>
                  <label className='settings-checkbox-row'>
                    <input
                      type='checkbox'
                      checked={prefs.launchAtLogin}
                      onChange={(e) => updateField('launchAtLogin', e.target.checked)}
                    />
                    <span>Launch at Login</span>
                  </label>
                </Tooltip>
              </div>
            </div>

            <div className='settings-card settings-card-wallpaper'>
              <div className='settings-card-title'>Wallpaper</div>
              <div className='settings-section'>
                <Tooltip text='How the image fits the screen.'>
                  <label className='settings-label'>Wallpaper Fit</label>
                </Tooltip>
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
                <Tooltip text='Rotate wallpapers from a folder.'>
                  <label className='settings-checkbox-row'>
                    <input
                      type='checkbox'
                      checked={prefs.wallpaper?.slideshowEnabled ?? false}
                      onChange={(e) => updateWallpaper('slideshowEnabled', e.target.checked)}
                    />
                    <span>Enable Wallpaper Slideshow</span>
                  </label>
                </Tooltip>
              </div>

              {prefs.wallpaper?.slideshowEnabled && (
                <>
                  <div className='settings-section'>
                    <Tooltip text='Image folder.'>
                      <label className='settings-label'>Slideshow Folder</label>
                    </Tooltip>
                    <input
                      type='text'
                      value={prefs.wallpaper?.slideshowFolder ?? ''}
                      placeholder='/path/to/wallpapers'
                      className='settings-text-input'
                      spellCheck={false}
                      onChange={(e) => updateWallpaper('slideshowFolder', e.target.value || null)}
                    />
                  </div>

                  <div className='settings-section'>
                    <Tooltip text='Time per wallpaper.'>
                      <label className='settings-label'>Interval</label>
                    </Tooltip>
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
                      Changes every{' '}
                      {formatInterval(intervalHours, intervalMinutes, intervalSeconds)}
                    </span>
                  </div>

                  <div className='settings-section'>
                    <Tooltip text='Image order.'>
                      <label className='settings-label'>Slideshow Order</label>
                    </Tooltip>
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
            </div>

            <div className='settings-card settings-card-night'>
              <div className='settings-card-title'>Night Mode</div>
              <div className='settings-section'>
                <Tooltip text='Auto-switch profiles by time of day.'>
                  <label className='settings-checkbox-row'>
                    <input
                      type='checkbox'
                      checked={schedule.enabled}
                      onChange={(e) => updateSchedule('enabled', e.target.checked)}
                    />
                    <span>Night Mode Schedule</span>
                  </label>
                </Tooltip>
              </div>

              {schedule.enabled && (
                <>
                  {renderScheduleRow('Night', 'nightStart', 'nightProfile')}
                  {renderScheduleRow('Day', 'dayStart', 'dayProfile')}
                </>
              )}
            </div>

            <div className='settings-card settings-card-profiles'>
              <Tooltip text='What each profile applies. Used by the schedule and Shift+F1 / Shift+F2.'>
                <div className='settings-card-title'>Profiles</div>
              </Tooltip>
              {editableProfileIndexes.length > 0 && (
                <>
                  <Dropdown
                    className='settings-dropdown'
                    aria-label='Profile to edit'
                    value={String(activeProfileIdx)}
                    onChange={(e) => setSelectedProfileIdx(Number(e.target.value))}>
                    {editableProfileIndexes.map((idx) => (
                      <option key={idx} value={idx}>
                        {prefs.profiles[idx].name || `Profile ${idx + 1}`}
                      </option>
                    ))}
                  </Dropdown>
                  {renderProfileEditor(activeProfileIdx)}
                </>
              )}
            </div>
          </>
        )}

        {activeTab === 'tiling' && (
          <>
            <div className='settings-card settings-card-tiling'>
              <div className='settings-card-title'>Window Tiling</div>
              <div className='settings-section'>
                <div className='settings-status-row'>
                  <Tooltip text='Shortcuts to move and resize windows.'>
                    <label className='settings-checkbox-row'>
                      <input
                        type='checkbox'
                        checked={tiling?.enabled ?? true}
                        onChange={(e) => updateTiling('enabled', e.target.checked)}
                      />
                      <span>Enable Window Tiling</span>
                    </label>
                  </Tooltip>
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
            </div>

            {tiling?.enabled && (
              <>
                <div className='settings-card settings-card-snap'>
                  <div className='settings-card-title'>Tile Snap</div>
                  <div className='settings-section'>
                    <div className='settings-status-row'>
                      <Tooltip text='Drag to an edge or corner to snap.'>
                        <label className='settings-checkbox-row'>
                          <input
                            type='checkbox'
                            checked={tiling.tileSnapEnabled}
                            onChange={(e) => updateTiling('tileSnapEnabled', e.target.checked)}
                          />
                          <span>Enable Tile Snap (drag to edge)</span>
                        </label>
                      </Tooltip>
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
                      <Tooltip text='Snap trigger distance, in pixels.'>
                        <label className='settings-label'>Snap Zones</label>
                      </Tooltip>
                      <div style={{ marginTop: '4px' }}>
                        <Tooltip text='Left/right edge distance.'>
                          <label className='settings-label'>Side Edge</label>
                        </Tooltip>
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
                        <Tooltip text='Top edge distance.'>
                          <label className='settings-label'>Top Edge</label>
                        </Tooltip>
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
                        <Tooltip text='Corner zone size.'>
                          <label className='settings-label'>Corner</label>
                        </Tooltip>
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
                        <Tooltip text='Turn snap zones on or off.'>
                          <label className='settings-label'>Zone Visibility</label>
                        </Tooltip>

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
                        <Tooltip text='Top edge: maximize.'>
                          <label className='settings-checkbox-row'>
                            <input
                              type='checkbox'
                              checked={tiling?.snapTopEdgeEnabled ?? true}
                              onChange={(e) => updateTiling('snapTopEdgeEnabled', e.target.checked)}
                            />
                            <span>Top edge (maximize)</span>
                          </label>
                        </Tooltip>
                        <Tooltip text='Left edge: left half.'>
                          <label className='settings-checkbox-row'>
                            <input
                              type='checkbox'
                              checked={tiling?.snapLeftEdgeEnabled ?? true}
                              onChange={(e) =>
                                updateTiling('snapLeftEdgeEnabled', e.target.checked)
                              }
                            />
                            <span>Left edge (left half)</span>
                          </label>
                        </Tooltip>
                        <Tooltip text='Right edge: right half.'>
                          <label className='settings-checkbox-row'>
                            <input
                              type='checkbox'
                              checked={tiling?.snapRightEdgeEnabled ?? true}
                              onChange={(e) =>
                                updateTiling('snapRightEdgeEnabled', e.target.checked)
                              }
                            />
                            <span>Right edge (right half)</span>
                          </label>
                        </Tooltip>

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
                        <Tooltip text='Top-left quarter.'>
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
                        </Tooltip>
                        <Tooltip text='Top-right quarter.'>
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
                        </Tooltip>
                        <Tooltip text='Bottom-left quarter.'>
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
                        </Tooltip>
                        <Tooltip text='Bottom-right quarter.'>
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
                        </Tooltip>

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
                        <Tooltip text='Bottom edge: thirds.'>
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
                        </Tooltip>
                        <Tooltip text='Bottom edge: two-thirds.'>
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
                        </Tooltip>
                      </div>
                    </div>
                  )}
                </div>

                <div className='settings-card settings-card-expose'>
                  <div className='settings-card-title'>Exposé</div>
                  <div className='settings-section'>
                    <Tooltip text='Show all windows in a grid.'>
                      <label className='settings-checkbox-row'>
                        <input
                          type='checkbox'
                          checked={tiling?.exposeEnabled ?? true}
                          onChange={(e) => updateTiling('exposeEnabled', e.target.checked)}
                        />
                        <span>Enable Exposé</span>
                      </label>
                    </Tooltip>
                  </div>

                  {(tiling?.exposeEnabled ?? true) && (
                    <div className='settings-section'>
                      <Tooltip text='Grid size per display.'>
                        <label className='settings-label'>Exposé Grid Size</label>
                      </Tooltip>
                      <div style={{ marginTop: '4px' }}>
                        <Tooltip text='Grid columns.'>
                          <label className='settings-label'>Columns</label>
                        </Tooltip>
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
                        <Tooltip text='Grid rows.'>
                          <label className='settings-label'>Rows</label>
                        </Tooltip>
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
                        <Tooltip text='How windows spread across displays.'>
                          <label className='settings-label'>Layout Strategy</label>
                        </Tooltip>
                        <Dropdown
                          className='settings-dropdown'
                          value={tiling?.exposeLayoutStrategy ?? 'spread'}
                          onChange={(e) => updateTiling('exposeLayoutStrategy', e.target.value)}>
                          <option value='spread'>Spread (distribute evenly across displays)</option>
                          <option value='fill'>Fill (pack each display before using next)</option>
                        </Dropdown>
                      </div>
                      <div style={{ marginTop: '8px' }}>
                        <Tooltip text='Min cell width; extras overflow.'>
                          <label className='settings-label'>Min Cell Width</label>
                        </Tooltip>
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
                        <Tooltip text='Min cell height; extras overflow.'>
                          <label className='settings-label'>Min Cell Height</label>
                        </Tooltip>
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
                </div>
              </>
            )}
          </>
        )}
      </div>
    </div>
  );
}
