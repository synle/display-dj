import { Icon } from './components/Icons';
import { useState, useEffect, useCallback, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow, LogicalSize } from '@tauri-apps/api/window';
import Header from './components/Header';
import AllMonitorsControl from './components/AllMonitorsControl';
import MonitorControl from './components/MonitorControl';
import VolumeControl from './components/VolumeControl';
import DarkModeToggle from './components/DarkModeToggle';
import ProfileButtons from './components/ProfileButtons';
import KeepAwakeToggle from './components/KeepAwakeToggle';
import SettingsPanel from './components/SettingsPanel';
import AboutPanel from './components/AboutPanel';
import AccessibilityGate from './components/AccessibilityGate';
import RefreshLabel from './components/RefreshLabel';
import { AudioOutputState, LastKnownValues, Monitor, Preferences, Profile } from './types';

const ABSOLUTE_MIN_BRIGHTNESS = 5;

/** Slider value rendered when no last-known value was recorded. */
const DEFAULT_SLIDER_VALUE = 50;

/** Delay before persisting last-known values, so a drag saves once it settles. */
const LAST_KNOWN_PERSIST_MS = 200;

/** Merges two last-known patches: scalars overwrite, per-id maps combine. */
function mergeLastKnown(a: LastKnownValues, b: LastKnownValues): LastKnownValues {
  const merged: LastKnownValues = { ...a, ...b };
  for (const key of ['monitorBrightness', 'monitorContrast', 'speakerVolume'] as const) {
    if (a[key] || b[key]) merged[key] = { ...a[key], ...b[key] };
  }
  return merged;
}

/** Root component: manages all app state (monitors, dark mode, volume, preferences)
 * and renders the main UI or settings panel. */
function App() {
  const [monitors, setMonitors] = useState<Monitor[]>([]);
  const [darkMode, setDarkMode] = useState(false);
  const [minBrightness, setMinBrightness] = useState(10);
  const [showContrast, setShowContrast] = useState(false);
  const [profiles, setProfiles] = useState<Profile[]>([]);
  const [keepAwake, setKeepAwake] = useState(false);
  const [monitorsExpanded, setMonitorsExpanded] = useState(false);
  const [speakersExpanded, setSpeakersExpanded] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [aboutOpen, setAboutOpen] = useState(false);
  const [version, setVersion] = useState('');
  const [isMac, setIsMac] = useState(false);
  const [accessibilityTrusted, setAccessibilityTrusted] = useState(true);
  const [audioOutputState, setAudioOutputState] = useState<AudioOutputState | null>(null);
  const [updatingAudioOutputId, setUpdatingAudioOutputId] = useState<string | null>(null);
  const [lastKnown, setLastKnown] = useState<LastKnownValues>({});
  const appRef = useRef<HTMLDivElement>(null);
  const audioOutputFetchInFlight = useRef(false);
  const audioOutputStateVersion = useRef(0);
  const pendingLastKnown = useRef<LastKnownValues | null>(null);
  const lastKnownTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  /** Sends the coalesced pending last-known patch; failures are ignored. */
  const flushLastKnown = useCallback(() => {
    if (lastKnownTimer.current) clearTimeout(lastKnownTimer.current);
    lastKnownTimer.current = null;
    const patch = pendingLastKnown.current;
    pendingLastKnown.current = null;
    if (patch) invoke('record_last_known_values', { patch }).catch(() => {});
  }, []);

  /** Queues last-known slider values; persisted once after 200ms of quiet. */
  const recordLastKnown = (patch: LastKnownValues) => {
    pendingLastKnown.current = pendingLastKnown.current
      ? mergeLastKnown(pendingLastKnown.current, patch)
      : patch;
    if (lastKnownTimer.current) clearTimeout(lastKnownTimer.current);
    lastKnownTimer.current = setTimeout(flushLastKnown, LAST_KNOWN_PERSIST_MS);
  };
  const showAccessibilityGate = isMac && !accessibilityTrusted;
  const mainViewVisible = !showAccessibilityGate && !aboutOpen && !settingsOpen;

  /** Fetches the list of connected monitors from the backend. */
  const fetchMonitors = useCallback(async () => {
    try {
      const m = await invoke<Monitor[]>('get_monitors');
      setMonitors(m);
    } catch {}
  }, []);

  /** Fetches the current dark mode state from the backend. */
  const fetchDarkMode = useCallback(async () => {
    try {
      const dm = await invoke<boolean>('get_dark_mode');
      setDarkMode(dm);
    } catch {}
  }, []);

  /** Fetches selectable audio outputs without overlapping slow platform probes. */
  const fetchAudioOutputs = useCallback(async () => {
    if (audioOutputFetchInFlight.current) return;
    audioOutputFetchInFlight.current = true;
    const requestVersion = audioOutputStateVersion.current;
    try {
      const outputState = await invoke<AudioOutputState>('get_audio_output_devices');
      if (requestVersion === audioOutputStateVersion.current) {
        setAudioOutputState(outputState);
      }
    } catch {
    } finally {
      audioOutputFetchInFlight.current = false;
    }
  }, []);

  /** Fetches the current keep-awake state from the backend. */
  const fetchKeepAwake = useCallback(async () => {
    try {
      const active = await invoke<boolean>('get_keep_awake');
      setKeepAwake(active);
    } catch {}
  }, []);

  /** Fetches user preferences (min brightness, profiles) from the backend. */
  const fetchPreferences = useCallback(async () => {
    try {
      const prefs = await invoke<Preferences>('get_preferences');
      setMinBrightness(Math.max(prefs.minBrightness, ABSOLUTE_MIN_BRIGHTNESS));
      setShowContrast(prefs.showContrast ?? false);
      setProfiles(prefs.profiles || []);
      setLastKnown(prefs.lastKnownValues ?? {});
    } catch {}
  }, []);

  /** Fetches monitors, dark mode, and volume in a single parallel backend call. */
  const fetchAllState = useCallback(async () => {
    try {
      const state = await invoke<{
        monitors: Monitor[];
        isDark: boolean;
      }>('fetch_all_state');
      setMonitors(state.monitors);
      setDarkMode(state.isDark);
    } catch {}
  }, []);

  useEffect(() => {
    // Startup state fetches. False positive below: each fn is async and every
    // setState runs after `await invoke`, so nothing renders synchronously
    // inside this effect's body.
    // oxlint-disable react/set-state-in-effect
    void Promise.all([fetchAllState(), fetchPreferences(), fetchKeepAwake()]);
    // oxlint-enable react/set-state-in-effect
    // Detect macOS once and track the Accessibility-permission state so we
    // can render a blocking gate when it's missing (tiling/Tile-Snap/exposé
    // silently no-op without it — see AGENTS.md "macOS Tray Icon Pitfall").
    invoke<Record<string, string>>('get_about_info')
      .then((info) => setIsMac(info.os === 'macOS'))
      .catch(() => {});
    invoke<boolean>('get_accessibility_trusted')
      .then((v) => setAccessibilityTrusted(v ?? true))
      .catch(() => {});
    invoke<string>('get_app_version')
      .then(setVersion)
      .catch(() => {});

    // Listen for backend events from shortcuts, tray actions, and refresh workers.
    const unlisten1 = listen('monitors-changed', () => fetchMonitors());
    const unlisten2 = listen('dark-mode-changed', () => fetchDarkMode());
    const unlisten4 = listen<AudioOutputState>('audio-output-changed', (event) => {
      setAudioOutputState(event.payload);
    });
    const unlisten5 = listen('show-about', () => {
      setAboutOpen(true);
      setSettingsOpen(false);
    });

    // Also refetch when window becomes visible
    const handleVisibility = () => {
      if (document.visibilityState === 'visible') {
        fetchAllState();
        fetchKeepAwake();
        // Use the cache-bypassing recheck on every popup open so a freshly
        // granted Accessibility permission auto-dismisses the gate without
        // the user having to click "I've granted it — recheck". When the
        // permission flips from false → true we also refetch the state that
        // was previously gated, so the normal popup body lands populated.
        invoke<boolean>('recheck_accessibility_trusted')
          .then((trusted) => {
            setAccessibilityTrusted((prev) => {
              if (trusted && !prev) {
                fetchAllState();
                fetchPreferences();
                fetchKeepAwake();
              }
              return trusted;
            });
          })
          .catch(() => {});
      }
    };
    document.addEventListener('visibilitychange', handleVisibility);

    // Close the About panel when the window loses focus (user clicks away)
    const handleBlur = () => {
      setAboutOpen(false);
      flushLastKnown();
    };
    window.addEventListener('blur', handleBlur);

    return () => {
      unlisten1.then((f) => f());
      unlisten2.then((f) => f());
      unlisten4.then((f) => f());
      unlisten5.then((f) => f());
      document.removeEventListener('visibilitychange', handleVisibility);
      window.removeEventListener('blur', handleBlur);
      flushLastKnown();
    };
  }, [
    fetchAllState,
    fetchMonitors,
    fetchDarkMode,
    fetchPreferences,
    fetchKeepAwake,
    flushLastKnown,
  ]);

  // The backend owns hot-plug probing. Poll its shared snapshot while the main
  // panel is visible as a fallback, retaining the last successful state.
  useEffect(() => {
    if (!mainViewVisible) return;

    let intervalId: number | null = null;
    const stopInterval = () => {
      if (intervalId !== null) {
        window.clearInterval(intervalId);
        intervalId = null;
      }
    };
    const startInterval = () => {
      if (document.visibilityState !== 'visible') {
        stopInterval();
        return;
      }
      void fetchAudioOutputs();
      if (intervalId === null) {
        intervalId = window.setInterval(() => {
          if (document.visibilityState === 'visible') {
            void fetchAudioOutputs();
          }
        }, 5_000);
      }
    };
    const handleVisibility = () => {
      if (document.visibilityState === 'visible') {
        startInterval();
      } else {
        stopInterval();
      }
    };

    document.addEventListener('visibilitychange', handleVisibility);
    startInterval();
    return () => {
      stopInterval();
      document.removeEventListener('visibilitychange', handleVisibility);
    };
  }, [fetchAudioOutputs, mainViewVisible]);

  // Auto-resize window to fit content
  useEffect(() => {
    const el = appRef.current;
    if (!el) return;
    const win = getCurrentWindow();
    const observer = new ResizeObserver(() => {
      const height = el.scrollHeight;
      if (height > 0) {
        const availableHeight = window.screen.availHeight || window.innerHeight;
        win.setSize(new LogicalSize(400, Math.min(height, availableHeight)));
      }
    });
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  /** Sets brightness for all monitors with optimistic UI update. */
  const handleAllBrightness = async (value: number) => {
    const targets = monitors.filter((m) => !m.hidden && m.supportsBrightness);
    const monitorBrightness = Object.fromEntries(targets.map((m) => [m.uid, value]));
    setLastKnown((prev) => ({
      ...prev,
      allBrightness: value,
      monitorBrightness: { ...prev.monitorBrightness, ...monitorBrightness },
    }));
    recordLastKnown({ allBrightness: value, monitorBrightness });
    try {
      await invoke('set_all_brightness', { value });
    } catch {
      // Keep the last-known value; hardware errors are not surfaced.
    }
  };

  /** Sets brightness for a single monitor with optimistic UI update. */
  const handleMonitorBrightness = async (monitorId: string, uid: string, value: number) => {
    setLastKnown((prev) => ({
      ...prev,
      monitorBrightness: { ...prev.monitorBrightness, [uid]: value },
    }));
    recordLastKnown({ monitorBrightness: { [uid]: value } });
    try {
      await invoke('set_brightness', { monitorId, value });
    } catch {
      // Keep the last-known value; hardware errors are not surfaced.
    }
  };

  /** Sets contrast for all monitors and records it as the last-known value. */
  const handleAllContrast = async (value: number) => {
    const targets = monitors.filter((m) => !m.hidden && m.contrast !== null);
    const monitorContrast = Object.fromEntries(targets.map((m) => [m.uid, value]));
    setLastKnown((prev) => ({
      ...prev,
      allContrast: value,
      monitorContrast: { ...prev.monitorContrast, ...monitorContrast },
    }));
    recordLastKnown({ allContrast: value, monitorContrast });
    try {
      await invoke('set_all_contrast', { value });
    } catch {
      // Keep the last-known value; hardware errors are not surfaced.
    }
  };

  /** Sets contrast for one monitor and records it as the last-known value. */
  const handleMonitorContrast = async (monitorId: string, uid: string, value: number) => {
    setLastKnown((prev) => ({
      ...prev,
      monitorContrast: { ...prev.monitorContrast, [uid]: value },
    }));
    recordLastKnown({ monitorContrast: { [uid]: value } });
    try {
      await invoke('set_contrast', { monitorId, value });
    } catch {
      // Keep the last-known value; hardware errors are not surfaced.
    }
  };

  /** Renames a monitor's display label via the backend. */
  const handleRename = async (uid: string, name: string) => {
    try {
      await invoke('rename_monitor', { uid, name });
      setMonitors((prev) => prev.map((m) => (m.uid === uid ? { ...m, name } : m)));
    } catch {}
  };

  /** Swaps a monitor's position in the list with its neighbor. */
  const handleReorder = async (index: number, direction: 'up' | 'down') => {
    const swapIndex = direction === 'up' ? index - 1 : index + 1;
    if (swapIndex < 0 || swapIndex >= monitors.length) return;

    const a = monitors[index];
    const b = monitors[swapIndex];

    try {
      await invoke('save_monitor_order', {
        orders: [
          [a.uid, swapIndex],
          [b.uid, index],
        ],
      });
      // Swap locally for instant feedback
      setMonitors((prev) => {
        const next = [...prev];
        next[index] = prev[swapIndex];
        next[swapIndex] = prev[index];
        return next;
      });
    } catch {}
  };

  /** Asks the backend to rescan displays and speakers; change events update the lists. */
  const handleRefreshDevices = () => {
    invoke('refresh_devices').catch(() => {});
  };

  /** Toggles dark/light mode via the backend. */
  const handleDarkMode = async (enabled: boolean) => {
    const previous = darkMode;
    setDarkMode(enabled);
    try {
      await invoke('set_dark_mode', { enabled });
    } catch {
      setDarkMode(previous);
    }
  };

  /** Sets the system volume via the backend. */
  const handleVolume = async (value: number) => {
    const speakerId = audioOutputState?.selectedDeviceId ?? null;
    const speakerVolume = speakerId ? { [speakerId]: value } : {};
    setLastKnown((prev) => ({
      ...prev,
      allVolume: value,
      speakerVolume: { ...prev.speakerVolume, ...speakerVolume },
    }));
    recordLastKnown({ allVolume: value, speakerVolume });
    try {
      await invoke('set_volume', { value });
    } catch {
      // Keep the last-known value; hardware errors are not surfaced.
    }
  };

  /** Selects a system audio output with optimistic radio-button feedback. */
  const handleAudioOutputSelect = async (id: string) => {
    const previousState = audioOutputState;
    audioOutputStateVersion.current += 1;
    setUpdatingAudioOutputId(id);
    setAudioOutputState((current) => (current ? { ...current, selectedDeviceId: id } : current));
    try {
      const outputState = await invoke<AudioOutputState>('set_audio_output_device', { id });
      setAudioOutputState(outputState);
      recordLastKnown({ selectedSpeakerId: id });
    } catch {
      setAudioOutputState(previousState);
    } finally {
      setUpdatingAudioOutputId(null);
    }
  };

  /** Saves or clears a Display DJ alias for one audio output. */
  const handleAudioOutputRename = async (id: string, label: string) => {
    audioOutputStateVersion.current += 1;
    try {
      await invoke('rename_audio_output_device', { id, label });
      setAudioOutputState((current) => {
        if (!current) return current;
        return {
          ...current,
          devices: current.devices.map((device) =>
            device.id === id ? { ...device, name: label.trim() || device.originalName } : device,
          ),
        };
      });
    } catch {}
  };

  /** Moves one speaker by one row and persists the complete device order. */
  const handleAudioOutputMove = async (id: string, direction: 'up' | 'down') => {
    if (!audioOutputState) return;
    audioOutputStateVersion.current += 1;
    const visibleDevices = audioOutputState.devices.filter((device) => device.state !== 'hidden');
    const visibleIndex = visibleDevices.findIndex((device) => device.id === id);
    const targetVisibleIndex = direction === 'up' ? visibleIndex - 1 : visibleIndex + 1;
    if (visibleIndex < 0 || targetVisibleIndex < 0 || targetVisibleIndex >= visibleDevices.length)
      return;
    const index = audioOutputState.devices.findIndex((device) => device.id === id);
    const swapIndex = audioOutputState.devices.findIndex(
      (device) => device.id === visibleDevices[targetVisibleIndex].id,
    );
    const devices = [...audioOutputState.devices];
    [devices[index], devices[swapIndex]] = [devices[swapIndex], devices[index]];
    const previousState = audioOutputState;
    setAudioOutputState({ ...audioOutputState, devices });
    try {
      const outputState = await invoke<AudioOutputState>('save_audio_output_order', {
        orderedIds: devices.map((device) => device.id),
      });
      setAudioOutputState(outputState);
    } catch {
      setAudioOutputState(previousState);
    }
  };

  /** Toggles the keep-awake state (prevents system from sleeping). */
  const handleKeepAwake = async (enabled: boolean) => {
    const previous = keepAwake;
    setKeepAwake(enabled);
    try {
      await invoke('set_keep_awake', { enabled });
    } catch {
      setKeepAwake(previous);
    }
  };

  /** Applies a saved profile by index and refreshes all state. */
  const handleProfile = async (index: number) => {
    try {
      await invoke('apply_profile', { index });
      await Promise.all([fetchMonitors(), fetchDarkMode()]);
    } catch {}
  };

  // macOS-only: when Accessibility permission is missing, render a blocking
  // gate instead of the normal popup body. Tiling, Tile Snap, exposé, and
  // z-order commands all no-op without it; surfacing the fix in the popup
  // (rather than only auto-opening System Settings on launch) makes the
  // recovery loop a single round-trip.
  // Only show non-hidden monitors in the main UI
  // Brightness, contrast, and volume render last-known values recorded from
  // this app (default 50%), never live hardware reads. A null live contrast
  // still means the monitor has no DDC contrast support.
  const visibleMonitors = monitors
    .filter((m) => !m.hidden)
    .map((m) => ({
      ...m,
      brightness: lastKnown.monitorBrightness?.[m.uid] ?? DEFAULT_SLIDER_VALUE,
      contrast:
        m.contrast === null ? null : (lastKnown.monitorContrast?.[m.uid] ?? DEFAULT_SLIDER_VALUE),
    }));
  const allBrightness = lastKnown.allBrightness ?? DEFAULT_SLIDER_VALUE;
  const selectedSpeakerId = audioOutputState?.selectedDeviceId ?? null;
  const volume = speakersExpanded
    ? ((selectedSpeakerId ? lastKnown.speakerVolume?.[selectedSpeakerId] : undefined) ??
      DEFAULT_SLIDER_VALUE)
    : (lastKnown.allVolume ?? DEFAULT_SLIDER_VALUE);

  // Whether any visible monitor supports contrast (used to show/hide the contrast slider)
  const hasContrast = visibleMonitors.some((m) => m.contrast !== null);
  const allContrast = lastKnown.allContrast ?? DEFAULT_SLIDER_VALUE;

  return (
    <div className='app' ref={appRef} data-theme={darkMode ? 'dark' : 'light'}>
      <Header
        version={version}
        onSettingsToggle={() => setSettingsOpen(!settingsOpen)}
        settingsOpen={settingsOpen}
      />

      {showAccessibilityGate ? (
        <AccessibilityGate
          onGranted={() => {
            setAccessibilityTrusted(true);
            fetchAllState();
            fetchPreferences();
            fetchKeepAwake();
          }}
        />
      ) : aboutOpen ? (
        <AboutPanel onClose={() => setAboutOpen(false)} />
      ) : settingsOpen ? (
        <SettingsPanel
          onClose={() => setSettingsOpen(false)}
          onPreferencesSaved={() => {
            fetchPreferences();
            fetchMonitors();
          }}
        />
      ) : (
        <div className='app-content'>
          {visibleMonitors.length > 0 &&
            (!monitorsExpanded ? (
              <AllMonitorsControl
                brightness={allBrightness}
                brightnessMixed={false}
                onBrightnessChange={handleAllBrightness}
                contrast={hasContrast ? allContrast : null}
                contrastMixed={false}
                onContrastChange={handleAllContrast}
                showContrast={showContrast}
                monitorCount={visibleMonitors.length}
                minBrightness={minBrightness}
                onExpand={() => setMonitorsExpanded(true)}
                onRefresh={handleRefreshDevices}
              />
            ) : (
              <div className='monitors-list' id='monitor-controls'>
                <div className='section-label-row'>
                  <RefreshLabel
                    text={`All Monitors (${visibleMonitors.length})`}
                    onRefresh={handleRefreshDevices}
                  />
                  <button
                    className='section-toggle'
                    onClick={() => setMonitorsExpanded(false)}
                    aria-expanded='true'
                    aria-controls='monitor-controls'
                    title='Show all monitors control'>
                    <span className='chevron expanded'>
                      <Icon name='chevronRight' size={14} />
                    </span>
                  </button>
                </div>
                {visibleMonitors.map((monitor, index) => (
                  <MonitorControl
                    key={monitor.uid}
                    monitor={monitor}
                    onBrightnessChange={(v) => handleMonitorBrightness(monitor.id, monitor.uid, v)}
                    onContrastChange={(v) => handleMonitorContrast(monitor.id, monitor.uid, v)}
                    showContrast={showContrast}
                    onRename={(name) => handleRename(monitor.uid, name)}
                    onMoveUp={() => handleReorder(index, 'up')}
                    onMoveDown={() => handleReorder(index, 'down')}
                    isFirst={index === 0}
                    isLast={index === visibleMonitors.length - 1}
                    minBrightness={minBrightness}
                  />
                ))}
              </div>
            ))}

          <VolumeControl
            value={volume}
            onChange={handleVolume}
            onRefresh={handleRefreshDevices}
            outputState={audioOutputState}
            expanded={speakersExpanded}
            onToggleExpanded={() => setSpeakersExpanded((current) => !current)}
            updatingDeviceId={updatingAudioOutputId}
            onSelectOutput={handleAudioOutputSelect}
            onRenameOutput={handleAudioOutputRename}
            onMoveOutput={handleAudioOutputMove}
          />
          <DarkModeToggle isDarkMode={darkMode} onChange={handleDarkMode} />
          <ProfileButtons profiles={profiles} onActivate={handleProfile} />
          <KeepAwakeToggle isActive={keepAwake} onChange={handleKeepAwake} />
        </div>
      )}
    </div>
  );
}

export default App;
