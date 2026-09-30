export interface Monitor {
  id: string;
  uid: string;
  name: string;
  originalName: string;
  brightness: number;
  contrast: number | null;
  supportsBrightness: boolean;
  isBuiltIn: boolean;
  hidden: boolean;
}

/**
 * Per-monitor brightness control strategy. One of:
 *  - `"auto"` (default) — DDC -> gamma -> soft-overlay fallback.
 *  - `"ddc"` — DDC/CI only, no overlay fallback.
 *  - `"gamma"` — `SetDeviceGammaRamp` only, no overlay fallback.
 *  - `"overlay"` — skip hardware paths entirely, dim with a transparent
 *    click-through window. Required for panels (e.g. some USB-C Samsung
 *    Smart Monitors on Intel Iris Xe) where both DDC and gamma are
 *    silently rejected by the driver.
 */
export type BrightnessMode = 'auto' | 'ddc' | 'gamma' | 'overlay';

export interface MonitorMetadata {
  uid: string;
  apiId: string;
  apiName: string;
  label: string;
  sortOrder: number;
  hidden: boolean;
  /** Per-monitor brightness control strategy. Defaults to "auto". */
  brightnessMode: BrightnessMode;
}

/** Display DJ availability for one system playback endpoint. */
export type AudioOutputDeviceState = 'enabled' | 'disabled' | 'hidden';

/** Selectable system playback endpoint. */
export interface AudioOutputDevice {
  /** Stable platform identifier used for selection and persisted aliases. */
  id: string;
  /** Display DJ label after applying a saved alias. */
  name: string;
  /** Native operating-system endpoint name. */
  originalName: string;
  /** User-controlled availability in Display DJ. */
  state: AudioOutputDeviceState;
  /** Whether the operating system identifies this as an integrated output. */
  isBuiltIn: boolean;
}

/** Current playback endpoints and the operating system's selected default. */
export interface AudioOutputState {
  devices: AudioOutputDevice[];
  selectedDeviceId: string | null;
}

/** User-defined settings for a stable audio-output device identifier. */
export interface AudioOutputMetadata {
  id: string;
  label: string;
  state: AudioOutputDeviceState;
  sortOrder: number | null;
}

export interface NightModeSchedule {
  enabled: boolean;
  nightStart: string;
  nightBrightness: number;
  dayStart: string;
  dayBrightness: number;
  /** Optional commands to run when night mode activates (replaces default brightness+dark). */
  nightCommands: string[];
  /** Optional commands to run when day mode activates (replaces default brightness+light). */
  dayCommands: string[];
  /** Profile name applied when night starts (default "Focus"). */
  nightProfile: string;
  /** Profile name applied when day starts (default "Daylight"). */
  dayProfile: string;
}

export interface TilingPreferences {
  enabled: boolean;
  halfRatio: number;
  thirdRatio: number;
  gap: number;
  tileSnapEnabled: boolean;
  sideEdgeTrigger: number;
  topEdgeTrigger: number;
  cornerTrigger: number;
  /** Tile Snap zone visibility: top edge (maximize). Default true. */
  snapTopEdgeEnabled: boolean;
  /** Tile Snap zone visibility: left edge (left-half snap). Default true. */
  snapLeftEdgeEnabled: boolean;
  /** Tile Snap zone visibility: right edge (right-half snap). Default true. */
  snapRightEdgeEnabled: boolean;
  /** Tile Snap zone visibility: top-left corner (top-left quarter). Default true. */
  snapTopLeftCornerEnabled: boolean;
  /** Tile Snap zone visibility: top-right corner (top-right quarter). Default true. */
  snapTopRightCornerEnabled: boolean;
  /** Tile Snap zone visibility: bottom-left corner. Default true. */
  snapBottomLeftCornerEnabled: boolean;
  /** Tile Snap zone visibility: bottom-right corner. Default true. */
  snapBottomRightCornerEnabled: boolean;
  /** Tile Snap zone visibility: bottom-row 1/3 markers (group). Default true. */
  snapBottomThirdsEnabled: boolean;
  /** Tile Snap zone visibility: bottom-row 2/3 markers (group, 2× wider than 1/3). Default true. */
  snapBottomTwoThirdsEnabled: boolean;
  exposeEnabled: boolean;
  exposeColumns: number;
  exposeRows: number;
  /** Exposé layout strategy: "spread" (even across displays) or "fill" (pack then overflow). */
  exposeLayoutStrategy: string;
  /** Exposé: minimum grid cell width in logical pixels. Scaled by DPI on Windows. */
  exposeMinWidth: number;
  /** Exposé: minimum grid cell height in logical pixels. Scaled by DPI on Windows. */
  exposeMinHeight: number;
}

/** A single rule within a layout preset: match windows by app name and apply a tiling layout. */
export interface LayoutRule {
  appMatch: string;
  layout: string;
  displayIndex: number | null;
}

/** A named window layout preset containing one or more layout rules. */
export interface LayoutPreset {
  name: string;
  rules: LayoutRule[];
}

/** Tracks the wallpaper path set on a specific monitor. */
export interface MonitorWallpaper {
  /** UID of the monitor (e.g. "1::Dell U2723QE"). */
  monitorUid: string;
  /** Path to the wallpaper file in the wallpapers directory. */
  wallpaperPath: string;
}

/** Wallpaper preferences: fit mode, current wallpaper state, and slideshow config. */
export interface WallpaperPreferences {
  /** How the wallpaper image fits the screen: fill, fit, stretch, center, tile. */
  fit: string;
  /** Path to the currently active wallpaper in our wallpapers directory (all-monitors). */
  currentWallpaperPath: string | null;
  /** Per-monitor wallpaper state. */
  perMonitorWallpapers: MonitorWallpaper[];
  /** Whether slideshow is enabled (resumes on app restart). */
  slideshowEnabled: boolean;
  /** Folder path for slideshow images. */
  slideshowFolder: string | null;
  /** Slideshow interval minutes component (combined with seconds). */
  slideshowIntervalMinutes: number;
  /** Extra seconds added to the slideshow interval (0-59; total minimum 5s). */
  slideshowIntervalSeconds: number;
  /** Slideshow cycling order: "forward", "backward", "random". */
  slideshowOrder: string;
}

export interface Preferences {
  showIndividualDisplays: boolean;
  minBrightness: number;
  keyBindings: KeyBinding[];
  profiles: Profile[];
  nightModeSchedule: NightModeSchedule;
  showContrast: boolean;
  /** Beta: show the per-display DPI scaling section in Settings. */
  showDpiSettings: boolean;
  /** Lowest DPI scale percent offered (absolute floor 50). */
  dpiMinPercent: number;
  /** Highest DPI scale percent offered (absolute ceiling 500). */
  dpiMaxPercent: number;
  /** Step interval for continuous-scale backends (1-100). */
  dpiStepPercent: number;
  debugLogging: boolean;
  launchAtLogin: boolean;
  /** Windows only: re-enable Loudness Equalization on the active speaker. */
  loudnessEqualizationPreferred: boolean;
  monitorConfigs: MonitorMetadata[];
  /** User-defined labels for audio output devices. */
  audioOutputConfigs: AudioOutputMetadata[];
  tiling: TilingPreferences;
  layoutPresets: LayoutPreset[];
  /** Wallpaper preferences: fit mode and current wallpaper path. */
  wallpaper: WallpaperPreferences;
  /** Last slider values set from Display DJ (backend-owned). */
  lastKnownValues?: LastKnownValues;
}

/** Last-known slider values rendered instead of live hardware reads. */
export interface LastKnownValues {
  allBrightness?: number | null;
  /** Keyed by monitor uid. */
  monitorBrightness?: Record<string, number>;
  allContrast?: number | null;
  /** Keyed by monitor uid. */
  monitorContrast?: Record<string, number>;
  allVolume?: number | null;
  /** Keyed by audio-output device ID. */
  speakerVolume?: Record<string, number>;
  selectedSpeakerId?: string | null;
}

export interface KeyBinding {
  key: string;
  command: string | string[];
}

export interface Profile {
  name: string;
  command: string | string[];
}

/** One display as reported by the DPI scaling backend (`get_dpi_displays`). */
export interface DpiDisplay {
  id: string;
  name: string;
  current: number | null;
  options: number[];
  /** True when any percent works (Linux/xrandr); UI builds options from min/max/step. */
  continuous: boolean;
}
