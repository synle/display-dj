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
import { Monitor, Preferences, Profile } from './types';

const ABSOLUTE_MIN_BRIGHTNESS = 5;

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
  const [expanded, setExpanded] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [aboutOpen, setAboutOpen] = useState(false);
  const [version, setVersion] = useState('');
  const appRef = useRef<HTMLDivElement>(null);

  /** Fetches the list of connected monitors from the backend. */
  const fetchMonitors = useCallback(async () => {
    try {
      const m = await invoke<Monitor[]>('get_monitors');
      setMonitors(m);
    } catch (e) {
      console.error('Failed to get monitors:', e);
    }
  }, []);

  /** Fetches the current dark mode state from the backend. */
  const fetchDarkMode = useCallback(async () => {
    try {
      const dm = await invoke<boolean>('get_dark_mode');
      setDarkMode(dm);
    } catch (e) {
      console.error('Failed to get dark mode:', e);
    }
  }, []);

  /** Fetches the current system volume from the backend. */
  const fetchVolume = useCallback(async () => {
    try {
      const v = await invoke<number>('get_volume');
      setVolume(v);
    } catch (e) {
      console.error('Failed to get volume:', e);
    }
  }, []);

  /** Fetches the current keep-awake state from the backend. */
  const fetchKeepAwake = useCallback(async () => {
    try {
      const active = await invoke<boolean>('get_keep_awake');
      setKeepAwake(active);
    } catch (e) {
      console.error('Failed to get keep awake:', e);
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
      // ignore
    }
  }, []);

  /** Fetches monitors, dark mode, and volume in a single parallel sidecar call. */
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
    } catch (e) {
      console.error('Failed to fetch all state:', e);
    }
  }, []);

  useEffect(() => {
    fetchAllState();
    fetchPreferences();
    fetchKeepAwake();
    invoke<boolean>('get_accessibility_trusted').catch(() => {});
    invoke<string>('get_app_version')
      .then(setVersion)
      .catch(() => {});

    // Listen for backend events (from keyboard shortcuts)
    const unlisten1 = listen('monitors-changed', () => fetchMonitors());
    const unlisten2 = listen('dark-mode-changed', () => fetchDarkMode());
    const unlisten3 = listen('volume-changed', () => fetchVolume());
    const unlisten4 = listen('show-about', () => {
      setAboutOpen(true);
      setSettingsOpen(false);
    });

    // Also refetch when window becomes visible
    const handleVisibility = () => {
      if (document.visibilityState === 'visible') {
        fetchAllState();
        fetchKeepAwake();
        invoke<boolean>('get_accessibility_trusted').catch(() => {});
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
      document.removeEventListener('visibilitychange', handleVisibility);
      window.removeEventListener('blur', handleBlur);
    };
  }, [fetchMonitors, fetchDarkMode, fetchVolume, fetchPreferences, fetchKeepAwake]);

  // Auto-resize window to fit content
  useEffect(() => {
    const el = appRef.current;
    if (!el) return;
    const win = getCurrentWindow();
    const observer = new ResizeObserver(() => {
      const height = el.scrollHeight;
      if (height > 0) {
        win.setSize(new LogicalSize(400, height));
      }
    });
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  /** Tracked value for the "all monitors" brightness slider.
   * Not derived from monitors to avoid jumpy recalculations on fetch. */
  const [allBrightness, setAllBrightness] = useState(50);

  /** Tracked value for the "all monitors" contrast slider. */
  const [allContrast, setAllContrast] = useState(50);

  /** Sets brightness for all monitors with optimistic UI update. */
  const handleAllBrightness = async (value: number) => {
    setAllBrightness(value);
    setMonitors((prev) => prev.map((m) => ({ ...m, brightness: value })));
    try {
      await invoke('set_all_brightness', { value });
    } catch (e) {
      console.error('Failed to set brightness:', e);
      fetchMonitors();
    }
  };

  /** Sets brightness for a single monitor with optimistic UI update. */
  const handleMonitorBrightness = async (monitorId: string, uid: string, value: number) => {
    setMonitors((prev) => prev.map((m) => (m.uid === uid ? { ...m, brightness: value } : m)));
    try {
      await invoke('set_brightness', { monitorId, value });
    } catch (e) {
      console.error('Failed to set brightness:', e);
      fetchMonitors();
    }
  };

  /** Sets contrast for all monitors with optimistic UI update. */
  const handleAllContrast = async (value: number) => {
    setAllContrast(value);
    setMonitors((prev) => prev.map((m) => (m.contrast !== null ? { ...m, contrast: value } : m)));
    try {
      await invoke('set_all_contrast', { value });
    } catch (e) {
      console.error('Failed to set contrast:', e);
      fetchMonitors();
    }
  };

  /** Sets contrast for a single monitor with optimistic UI update. */
  const handleMonitorContrast = async (monitorId: string, uid: string, value: number) => {
    setMonitors((prev) => prev.map((m) => (m.uid === uid ? { ...m, contrast: value } : m)));
    try {
      await invoke('set_contrast', { monitorId, value });
    } catch (e) {
      console.error('Failed to set contrast:', e);
      fetchMonitors();
    }
  };

  /** Renames a monitor's display label via the backend. */
  const handleRename = async (uid: string, name: string) => {
    try {
      await invoke('rename_monitor', { uid, name });
      setMonitors((prev) => prev.map((m) => (m.uid === uid ? { ...m, name } : m)));
    } catch (e) {
      console.error('Failed to rename monitor:', e);
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
    } catch (e) {
      console.error('Failed to reorder monitors:', e);
    }
  };

  /** Toggles dark/light mode via the backend. */
  const handleDarkMode = async (enabled: boolean) => {
    try {
      await invoke('set_dark_mode', { enabled });
      setDarkMode(enabled);
    } catch (e) {
      console.error('Failed to set dark mode:', e);
    }
  };

  /** Sets the system volume via the backend. */
  const handleVolume = async (value: number) => {
    try {
      await invoke('set_volume', { value });
      setVolume(value);
    } catch (e) {
      console.error('Failed to set volume:', e);
    }
  };

  /** Toggles the keep-awake state (prevents system from sleeping). */
  const handleKeepAwake = async (enabled: boolean) => {
    try {
      await invoke('set_keep_awake', { enabled });
      setKeepAwake(enabled);
    } catch (e) {
      console.error('Failed to set keep awake:', e);
    }
  };

  /** Applies a saved profile by index and refreshes all state. */
  const handleProfile = async (index: number) => {
    try {
      await invoke('apply_profile', { index });
      fetchMonitors();
      fetchDarkMode();
      fetchVolume();
    } catch (e) {
      console.error('Failed to apply profile:', e);
    }
  };

  // Only show non-hidden monitors in the main UI
  const visibleMonitors = monitors.filter((m) => !m.hidden);

  // Whether any visible monitor supports contrast (used to show/hide the contrast slider)
  const hasContrast = visibleMonitors.some((m) => m.contrast !== null);

  return (
    <div className='app' ref={appRef} data-theme={darkMode ? 'dark' : 'light'}>
      <Header
        version={version}
        onSettingsToggle={() => setSettingsOpen(!settingsOpen)}
        settingsOpen={settingsOpen}
      />

      {aboutOpen ? (
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
            (!expanded ? (
              <AllMonitorsControl
                brightness={allBrightness}
                onBrightnessChange={handleAllBrightness}
                contrast={hasContrast ? allContrast : null}
                onContrastChange={handleAllContrast}
                showContrast={showContrast}
                monitorCount={visibleMonitors.length}
                minBrightness={minBrightness}
                onExpand={() => setExpanded(true)}
              />
            ) : (
              <div className='monitors-list'>
                <div className='section-label-row'>
                  <span className='section-label'>All Monitors ({visibleMonitors.length})</span>
                  <button
                    className='section-toggle'
                    onClick={() => setExpanded(false)}
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

          <VolumeControl value={volume} onChange={handleVolume} />
          <DarkModeToggle isDarkMode={darkMode} onChange={handleDarkMode} />
          <ProfileButtons profiles={profiles} onActivate={handleProfile} />
          <KeepAwakeToggle isActive={keepAwake} onChange={handleKeepAwake} />
        </div>
      )}
    </div>
  );
}

export default App;
