import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Preferences, NightModeSchedule } from "../types";
import Slider from "./Slider";

interface SettingsPanelProps {
  onClose: () => void;
  onPreferencesSaved: () => void;
}

export default function SettingsPanel({
  onClose,
  onPreferencesSaved,
}: SettingsPanelProps) {
  const [prefs, setPrefs] = useState<Preferences | null>(null);

  useEffect(() => {
    invoke<Preferences>("get_preferences")
      .then((p) => {
        // Clamp values to UI slider ranges in case saved values are out of bounds
        p.minBrightness = Math.max(5, Math.min(100, p.minBrightness));
        setPrefs(p);
      })
      .catch(console.error);
  }, []);

  if (!prefs) return null;

  const schedule = prefs.nightModeSchedule;

  const updateField = <K extends keyof Preferences>(
    key: K,
    value: Preferences[K]
  ) => {
    setPrefs((prev) => (prev ? { ...prev, [key]: value } : prev));
  };

  const updateSchedule = <K extends keyof NightModeSchedule>(
    key: K,
    value: NightModeSchedule[K]
  ) => {
    setPrefs((prev) =>
      prev
        ? {
            ...prev,
            nightModeSchedule: { ...prev.nightModeSchedule, [key]: value },
          }
        : prev
    );
  };

  const handleSave = async () => {
    if (!prefs) return;
    try {
      await invoke("save_preferences", { preferences: prefs });
      onPreferencesSaved();
      onClose();
    } catch (e) {
      console.error("Failed to save preferences:", e);
    }
  };

  return (
    <div className="settings-panel">
      <div className="settings-header">
        <span className="settings-title">Settings</span>
        <button className="settings-close" onClick={onClose} title="Close">
          &times;
        </button>
      </div>

      <div className="settings-body">
        <div className="settings-section">
          <label className="settings-label">Min Brightness</label>
          <Slider
            value={prefs.minBrightness}
            min={5}
            max={100}
            onChange={(v) => updateField("minBrightness", v)}
          />
        </div>

        <div className="settings-divider" />

        <div className="settings-section">
          <label className="settings-checkbox-row">
            <input
              type="checkbox"
              checked={schedule.enabled}
              onChange={(e) => updateSchedule("enabled", e.target.checked)}
            />
            <span>Night Mode Schedule</span>
          </label>
        </div>

        {schedule.enabled && (
          <>
            <div className="settings-section">
              <div className="settings-schedule-header">
                <label className="settings-label">Night</label>
                <input
                  type="time"
                  className="settings-time-input"
                  value={schedule.nightStart}
                  onChange={(e) => updateSchedule("nightStart", e.target.value)}
                />
              </div>
              <Slider
                value={schedule.nightBrightness}
                min={5}
                max={100}
                onChange={(v) => updateSchedule("nightBrightness", v)}
              />
            </div>

            <div className="settings-section">
              <div className="settings-schedule-header">
                <label className="settings-label">Day</label>
                <input
                  type="time"
                  className="settings-time-input"
                  value={schedule.dayStart}
                  onChange={(e) => updateSchedule("dayStart", e.target.value)}
                />
              </div>
              <Slider
                value={schedule.dayBrightness}
                min={5}
                max={100}
                onChange={(v) => updateSchedule("dayBrightness", v)}
              />
            </div>
          </>
        )}

        <div className="settings-divider" />

        <div className="settings-section">
          <label className="settings-checkbox-row">
            <input
              type="checkbox"
              checked={prefs.launchAtLogin}
              onChange={(e) => updateField("launchAtLogin", e.target.checked)}
            />
            <span>Launch at Login</span>
          </label>
        </div>

        <div className="settings-section">
          <label className="settings-checkbox-row">
            <input
              type="checkbox"
              checked={prefs.debugLogging}
              onChange={(e) => updateField("debugLogging", e.target.checked)}
            />
            <span>Debug Logging</span>
          </label>
        </div>
      </div>

      <div className="settings-footer">
        <button className="settings-btn settings-btn-cancel" onClick={onClose}>
          Cancel
        </button>
        <button className="settings-btn settings-btn-save" onClick={handleSave}>
          Save
        </button>
      </div>
    </div>
  );
}
