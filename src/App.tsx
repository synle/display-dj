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
import { AudioOutputState, Monitor, Preferences, Profile } from './types';

const ABSOLUTE_MIN_BRIGHTNESS = 5;

interface Feedback {
  kind: 'status' | 'error';
  message: string;
}

/** Root component: manages all app state (monitors, dark mode, volume, preferences)
 * and renders the main UI or settings panel. */
function App() {
  const [monitors, setMonitors] = useState<Monitor[]>([]);
  const [darkMode, setDarkMode] = useState(false);
  const [volume, setVolume] = useState(50);
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
  const [feedback, setFeedback] = useState<Feedback>({
    kind: 'status',
    message: 'Loading controls...',
  });
  const appRef = useRef<HTMLDivElement>(null);
  const audioOutputFetchInFlight = useRef(false);
  const audioOutputStateVersion = useRef(0);
  const showAccessibilityGate = isMac && !accessibilityTrusted;
  const mainViewVisible = !showAccessibilityGate && !aboutOpen && !settingsOpen;

  /** Fetches the list of connected monitors from the backend. */
  const fetchMonitors = useCallback(async () => {
    try {
      const m = await invoke<Monitor[]>('get_monitors');
      setMonitors(m);
    } catch {
      setFeedback({ kind: 'error', message: 'Could not refresh monitors.' });
    }
  }, []);

  /** Fetches the current dark mode state from the backend. */
  const fetchDarkMode = useCallback(async () => {
    try {
      const dm = await invoke<boolean>('get_dark_mode');
      setDarkMode(dm);
    } catch {
      setFeedback({ kind: 'error', message: 'Could not refresh dark mode.' });
    }
  }, []);

  /** Fetches the current system volume from the backend. */
  const fetchVolume = useCallback(async () => {
    try {
      const v = await invoke<number>('get_volume');
      setVolume(v);
    } catch {
      setFeedback({ kind: 'error', message: 'Could not refresh volume.' });
    }
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
      setFeedback({ kind: 'error', message: 'Could not load controls.' });
    } finally {
      audioOutputFetchInFlight.current = false;
    }
  }, []);

  /** Fetches the current keep-awake state from the backend. */
  const fetchKeepAwake = useCallback(async () => {
    try {
      const active = await invoke<boolean>('get_keep_awake');
      setKeepAwake(active);
    } catch {
      setFeedback({ kind: 'error', message: 'Could not refresh Keep Awake.' });
    }
  }, []);

  /** Fetches user preferences (min brightness, profiles) from the backend. */
  const fetchPreferences = useCallback(async () => {
    try {
      const prefs = await invoke<Preferences>('get_preferences');
      setMinBrightness(Math.max(prefs.minBrightness, ABSOLUTE_MIN_BRIGHTNESS));
      setShowContrast(prefs.showContrast ?? false);
      setProfiles(prefs.profiles || []);
    } catch {
      setFeedback({ kind: 'error', message: 'Could not load preferences.' });
    }
  }, []);

  /** Fetches monitors, dark mode, and volume in a single parallel backend call. */
  const fetchAllState = useCallback(async () => {
    try {
      const state = await invoke<{
        monitors: Monitor[];
        isDark: boolean;
        volume: number;
      }>('fetch_all_state');
      setMonitors(state.monitors);
      setDarkMode(state.isDark);
      setVolume(state.volume);
      setFeedback({ kind: 'status', message: '' });
    } catch {
      setFeedback({ kind: 'error', message: 'Could not load controls.' });
    }
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
    const unlisten3 = listen('volume-changed', () => fetchVolume());
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
    };
    window.addEventListener('blur', handleBlur);

    return () => {
      unlisten1.then((f) => f());
      unlisten2.then((f) => f());
      unlisten3.then((f) => f());
      unlisten4.then((f) => f());
      unlisten5.then((f) => f());
      document.removeEventListener('visibilitychange', handleVisibility);
      window.removeEventListener('blur', handleBlur);
    };
  }, [fetchAllState, fetchMonitors, fetchDarkMode, fetchVolume, fetchPreferences, fetchKeepAwake]);

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
    const previous = monitors;
    setFeedback({ kind: 'status', message: 'Updating brightness...' });
    setMonitors((prev) =>
      prev.map((m) => (m.hidden || !m.supportsBrightness ? m : { ...m, brightness: value })),
    );
    try {
      await invoke('set_all_brightness', { value });
      setFeedback({ kind: 'status', message: 'Brightness updated.' });
    } catch {
      setMonitors(previous);
      setFeedback({ kind: 'error', message: 'Could not update brightness.' });
    }
  };

  /** Sets brightness for a single monitor with optimistic UI update. */
  const handleMonitorBrightness = async (monitorId: string, uid: string, value: number) => {
    const previous = monitors;
    setFeedback({ kind: 'status', message: 'Updating brightness...' });
    setMonitors((prev) => prev.map((m) => (m.uid === uid ? { ...m, brightness: value } : m)));
    try {
      await invoke('set_brightness', { monitorId, value });
      setFeedback({ kind: 'status', message: 'Brightness updated.' });
    } catch {
      setMonitors(previous);
      setFeedback({ kind: 'error', message: 'Could not update brightness.' });
    }
  };

  /** Sets contrast for all monitors with optimistic UI update. */
  const handleAllContrast = async (value: number) => {
    const previous = monitors;
    setFeedback({ kind: 'status', message: 'Updating contrast...' });
    setMonitors((prev) => prev.map((m) => (m.contrast !== null ? { ...m, contrast: value } : m)));
    try {
      await invoke('set_all_contrast', { value });
      setFeedback({ kind: 'status', message: 'Contrast updated.' });
    } catch {
      setMonitors(previous);
      setFeedback({ kind: 'error', message: 'Could not update contrast.' });
    }
  };

  /** Sets contrast for a single monitor with optimistic UI update. */
  const handleMonitorContrast = async (monitorId: string, uid: string, value: number) => {
    const previous = monitors;
    setFeedback({ kind: 'status', message: 'Updating contrast...' });
    setMonitors((prev) => prev.map((m) => (m.uid === uid ? { ...m, contrast: value } : m)));
    try {
      await invoke('set_contrast', { monitorId, value });
      setFeedback({ kind: 'status', message: 'Contrast updated.' });
    } catch {
      setMonitors(previous);
      setFeedback({ kind: 'error', message: 'Could not update contrast.' });
    }
  };

  /** Renames a monitor's display label via the backend. */
  const handleRename = async (uid: string, name: string) => {
    try {
      await invoke('rename_monitor', { uid, name });
      setMonitors((prev) => prev.map((m) => (m.uid === uid ? { ...m, name } : m)));
      setFeedback({ kind: 'status', message: 'Monitor renamed.' });
    } catch {
      setFeedback({ kind: 'error', message: 'Could not rename monitor.' });
    }
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
    } catch {
      setFeedback({ kind: 'error', message: 'Could not reorder monitors.' });
    }
  };

  /** Toggles dark/light mode via the backend. */
  const handleDarkMode = async (enabled: boolean) => {
    const previous = darkMode;
    setDarkMode(enabled);
    setFeedback({ kind: 'status', message: 'Updating appearance...' });
    try {
      await invoke('set_dark_mode', { enabled });
      setFeedback({ kind: 'status', message: 'Appearance updated.' });
    } catch {
      setDarkMode(previous);
      setFeedback({ kind: 'error', message: 'Could not update appearance.' });
    }
  };

  /** Sets the system volume via the backend. */
  const handleVolume = async (value: number) => {
    const previous = volume;
    setVolume(value);
    setFeedback({ kind: 'status', message: 'Updating volume...' });
    try {
      await invoke('set_volume', { value });
      setFeedback({ kind: 'status', message: 'Volume updated.' });
    } catch {
      setVolume(previous);
      setFeedback({ kind: 'error', message: 'Could not update volume.' });
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
      await fetchVolume();
    } catch {
      setAudioOutputState(previousState);
      setFeedback({ kind: 'error', message: 'Could not change speaker.' });
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
    } catch {
      setFeedback({ kind: 'error', message: 'Could not rename speaker.' });
    }
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
      setFeedback({ kind: 'error', message: 'Could not reorder speakers.' });
    }
  };

  /** Toggles the keep-awake state (prevents system from sleeping). */
  const handleKeepAwake = async (enabled: boolean) => {
    const previous = keepAwake;
    setKeepAwake(enabled);
    setFeedback({ kind: 'status', message: 'Updating Keep Awake...' });
    try {
      await invoke('set_keep_awake', { enabled });
      setFeedback({ kind: 'status', message: 'Keep Awake updated.' });
    } catch {
      setKeepAwake(previous);
      setFeedback({ kind: 'error', message: 'Could not update Keep Awake.' });
    }
  };

  /** Applies a saved profile by index and refreshes all state. */
  const handleProfile = async (index: number) => {
    setFeedback({ kind: 'status', message: 'Applying profile...' });
    try {
      await invoke('apply_profile', { index });
      setFeedback({ kind: 'status', message: 'Profile applied.' });
      await Promise.all([fetchMonitors(), fetchDarkMode(), fetchVolume()]);
    } catch {
      setFeedback({ kind: 'error', message: 'Could not apply profile.' });
    }
  };

  // macOS-only: when Accessibility permission is missing, render a blocking
  // gate instead of the normal popup body. Tiling, Tile Snap, exposé, and
  // z-order commands all no-op without it; surfacing the fix in the popup
  // (rather than only auto-opening System Settings on launch) makes the
  // recovery loop a single round-trip.
  // Only show non-hidden monitors in the main UI
  const visibleMonitors = monitors.filter((m) => !m.hidden);
  const brightnessMonitors = visibleMonitors.filter((m) => m.supportsBrightness);
  const brightnessValues = brightnessMonitors.map((m) => m.brightness);
  const allBrightness = brightnessValues.length
    ? Math.round(brightnessValues.reduce((sum, value) => sum + value, 0) / brightnessValues.length)
    : minBrightness;
  const brightnessMixed = brightnessValues.some((value) => value !== brightnessValues[0]);

  // Whether any visible monitor supports contrast (used to show/hide the contrast slider)
  const contrastValues = visibleMonitors.flatMap((m) => (m.contrast === null ? [] : [m.contrast]));
  const hasContrast = contrastValues.length > 0;
  const allContrast = hasContrast
    ? Math.round(contrastValues.reduce((sum, value) => sum + value, 0) / contrastValues.length)
    : 0;
  const contrastMixed = contrastValues.some((value) => value !== contrastValues[0]);

  return (
    <div className='app' ref={appRef} data-theme={darkMode ? 'dark' : 'light'}>
      <Header
        version={version}
        onSettingsToggle={() => setSettingsOpen(!settingsOpen)}
        settingsOpen={settingsOpen}
      />

      {feedback.message && (
        <div
          className={`status-message status-message-${feedback.kind}`}
          role={feedback.kind === 'error' ? 'alert' : 'status'}
          aria-live={feedback.kind === 'error' ? 'assertive' : 'polite'}>
          <span>{feedback.message}</span>
          {feedback.message === 'Could not load controls.' && (
            <button onClick={() => void fetchAllState()}>Retry</button>
          )}
        </div>
      )}

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
                brightnessMixed={brightnessMixed}
                onBrightnessChange={handleAllBrightness}
                contrast={hasContrast ? allContrast : null}
                contrastMixed={contrastMixed}
                onContrastChange={handleAllContrast}
                showContrast={showContrast}
                monitorCount={visibleMonitors.length}
                minBrightness={minBrightness}
                onExpand={() => setMonitorsExpanded(true)}
              />
            ) : (
              <div className='monitors-list' id='monitor-controls'>
                <div className='section-label-row'>
                  <span className='section-label'>All Monitors ({visibleMonitors.length})</span>
                  <button
                    className='section-toggle'
                    onClick={() => setMonitorsExpanded(false)}
                    aria-expanded='true'
                    aria-controls='monitor-controls'
                    title='Show all monitors control'>
                    <span className='chevron expanded'>&#9662;</span>
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
