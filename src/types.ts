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

export interface MonitorMetadata {
  uid: string;
  apiId: string;
  apiName: string;
  label: string;
  sortOrder: number;
  hidden: boolean;
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
  /** Slideshow interval in minutes (minimum 5). */
  slideshowIntervalMinutes: number;
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
  debugLogging: boolean;
  launchAtLogin: boolean;
  monitorConfigs: MonitorMetadata[];
  tiling: TilingPreferences;
  layoutPresets: LayoutPreset[];
  /** Wallpaper preferences: fit mode and current wallpaper path. */
  wallpaper: WallpaperPreferences;
}

export interface KeyBinding {
  key: string;
  command: string | string[];
}

export interface Profile {
  name: string;
  command: string | string[];
}
