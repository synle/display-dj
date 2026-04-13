import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import Header from "./components/Header";
import AllMonitorsControl from "./components/AllMonitorsControl";
import MonitorControl from "./components/MonitorControl";
import VolumeControl from "./components/VolumeControl";
import DarkModeToggle from "./components/DarkModeToggle";
import ProfileButtons from "./components/ProfileButtons";
import SettingsPanel from "./components/SettingsPanel";
import { Monitor, Preferences, Profile } from "./types";

const ABSOLUTE_MIN_BRIGHTNESS = 5;

function App() {
  const [monitors, setMonitors] = useState<Monitor[]>([]);
  const [darkMode, setDarkMode] = useState(false);
  const [volume, setVolume] = useState(50);
  const [minBrightness, setMinBrightness] = useState(10);
  const [profiles, setProfiles] = useState<Profile[]>([]);
  const [expanded, setExpanded] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [version, setVersion] = useState("");
  const appRef = useRef<HTMLDivElement>(null);

  const fetchMonitors = useCallback(async () => {
    try {
      const m = await invoke<Monitor[]>("get_monitors");
      setMonitors(m);
    } catch (e) {
      console.error("Failed to get monitors:", e);
    }
  }, []);

  const fetchDarkMode = useCallback(async () => {
    try {
      const dm = await invoke<boolean>("get_dark_mode");
      setDarkMode(dm);
    } catch (e) {
      console.error("Failed to get dark mode:", e);
    }
  }, []);

  const fetchVolume = useCallback(async () => {
    try {
      const v = await invoke<number>("get_volume");
      setVolume(v);
    } catch (e) {
      console.error("Failed to get volume:", e);
    }
  }, []);

  const fetchPreferences = useCallback(async () => {
    try {
      const prefs = await invoke<Preferences>("get_preferences");
      setMinBrightness(Math.max(prefs.minBrightness, ABSOLUTE_MIN_BRIGHTNESS));
      setProfiles(prefs.profiles || []);
    } catch {
      // ignore
    }
  }, []);

  useEffect(() => {
    fetchMonitors();
    fetchDarkMode();
    fetchVolume();
    fetchPreferences();
    invoke<string>("get_app_version").then(setVersion).catch(() => {});

    // Listen for backend events (from keyboard shortcuts)
    const unlisten1 = listen("monitors-changed", () => fetchMonitors());
    const unlisten2 = listen("dark-mode-changed", () => fetchDarkMode());
    const unlisten3 = listen("volume-changed", () => fetchVolume());

    // Also refetch when window becomes visible
    const handleVisibility = () => {
      if (document.visibilityState === "visible") {
        fetchMonitors();
        fetchDarkMode();
        fetchVolume();
      }
    };
    document.addEventListener("visibilitychange", handleVisibility);

    return () => {
      unlisten1.then((f) => f());
      unlisten2.then((f) => f());
      unlisten3.then((f) => f());
      document.removeEventListener("visibilitychange", handleVisibility);
    };
  }, [fetchMonitors, fetchDarkMode, fetchVolume, fetchPreferences]);

  // Auto-resize window to fit content
  useEffect(() => {
    const el = appRef.current;
    if (!el) return;
    const win = getCurrentWindow();
    const observer = new ResizeObserver(() => {
      const height = el.scrollHeight;
      if (height > 0) {
        win.setSize(new LogicalSize(360, height));
      }
    });
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  const handleAllBrightness = async (value: number) => {
    setMonitors((prev) => prev.map((m) => ({ ...m, brightness: value })));
    try {
      await invoke("set_all_brightness", { value });
    } catch (e) {
      console.error("Failed to set brightness:", e);
      fetchMonitors();
    }
  };

  const handleMonitorBrightness = async (
    monitorId: string,
    uid: string,
    value: number
  ) => {
    setMonitors((prev) =>
      prev.map((m) => (m.uid === uid ? { ...m, brightness: value } : m))
    );
    try {
      await invoke("set_brightness", { monitorId, value });
    } catch (e) {
      console.error("Failed to set brightness:", e);
      fetchMonitors();
    }
  };

  const handleRename = async (uid: string, name: string) => {
    try {
      await invoke("rename_monitor", { uid, name });
      setMonitors((prev) =>
        prev.map((m) => (m.uid === uid ? { ...m, name } : m))
      );
    } catch (e) {
      console.error("Failed to rename monitor:", e);
    }
  };

  const handleReorder = async (index: number, direction: "up" | "down") => {
    const swapIndex = direction === "up" ? index - 1 : index + 1;
    if (swapIndex < 0 || swapIndex >= monitors.length) return;

    const a = monitors[index];
    const b = monitors[swapIndex];

    try {
      await invoke("save_monitor_order", {
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
      console.error("Failed to reorder monitors:", e);
    }
  };

  const handleDarkMode = async (enabled: boolean) => {
    try {
      await invoke("set_dark_mode", { enabled });
      setDarkMode(enabled);
    } catch (e) {
      console.error("Failed to set dark mode:", e);
    }
  };

  const handleVolume = async (value: number) => {
    try {
      await invoke("set_volume", { value });
      setVolume(value);
    } catch (e) {
      console.error("Failed to set volume:", e);
    }
  };

  const handleProfile = async (index: number) => {
    try {
      await invoke("apply_profile", { index });
      fetchMonitors();
      fetchDarkMode();
      fetchVolume();
    } catch (e) {
      console.error("Failed to apply profile:", e);
    }
  };

  // Only show non-hidden monitors in the main UI
  const visibleMonitors = monitors.filter((m) => !m.hidden);

  // Calculate average brightness for "all monitors" view
  const avgBrightness = visibleMonitors.length
    ? Math.round(
        visibleMonitors.reduce((sum, m) => sum + m.brightness, 0) / visibleMonitors.length
      )
    : 50;

  return (
    <div className="app" ref={appRef} data-theme={darkMode ? "dark" : "light"}>
      <Header
        version={version}
        expanded={expanded}
        onToggle={() => setExpanded(!expanded)}
        onSettingsToggle={() => setSettingsOpen(!settingsOpen)}
        settingsOpen={settingsOpen}
      />

      {settingsOpen ? (
        <SettingsPanel
          onClose={() => setSettingsOpen(false)}
          onPreferencesSaved={() => { fetchPreferences(); fetchMonitors(); }}
        />
      ) : (
      <div className="app-content">
        {visibleMonitors.length > 0 && (
          !expanded ? (
            <AllMonitorsControl
              brightness={avgBrightness}
              onBrightnessChange={handleAllBrightness}
              monitorCount={visibleMonitors.length}
              minBrightness={minBrightness}
            />
          ) : (
            <div className="monitors-list">
              {visibleMonitors.map((monitor, index) => (
                <MonitorControl
                  key={monitor.uid}
                  monitor={monitor}
                  onBrightnessChange={(v) =>
                    handleMonitorBrightness(monitor.id, monitor.uid, v)
                  }
                  onRename={(name) => handleRename(monitor.uid, name)}
                  onMoveUp={() => handleReorder(index, "up")}
                  onMoveDown={() => handleReorder(index, "down")}
                  isFirst={index === 0}
                  isLast={index === visibleMonitors.length - 1}
                  minBrightness={minBrightness}
                />
              ))}
            </div>
          )
        )}

        <VolumeControl value={volume} onChange={handleVolume} />
        <DarkModeToggle isDarkMode={darkMode} onChange={handleDarkMode} />
        <ProfileButtons profiles={profiles} onActivate={handleProfile} />
      </div>
      )}
    </div>
  );
}

export default App;
