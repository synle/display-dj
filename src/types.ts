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

/** Wallpaper preferences: fit mode and current wallpaper state. */
export interface WallpaperPreferences {
  /** How the wallpaper image fits the screen: fill, fit, stretch, center, tile. */
  fit: string;
  /** Path to the currently active wallpaper in our wallpapers directory. */
  currentWallpaperPath: string | null;
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
