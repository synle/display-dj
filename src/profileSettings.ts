import type { Profile } from './types';

/** Theme a profile switches to, or `null` when the profile leaves the theme alone. */
export type ProfileTheme = 'dark' | 'light' | null;

/** Structured view of the all-display settings a profile applies. */
export interface ProfileSettings {
  theme: ProfileTheme;
  /** All-display brightness percent, or `null` when unchanged. */
  brightness: number | null;
  /** System volume percent, or `null` when unchanged. */
  volume: number | null;
}

const BRIGHTNESS_RE = /^command\/changeBrightness\/(\d+)$/;
const VOLUME_RE = /^command\/changeVolume\/(\d+)$/;
const THEME_RE = /^command\/changeDarkMode\/(dark|light)$/;

/**
 * Normalizes a profile's command value into an array.
 * @param profile - profile whose `command` may be a string or string[]
 * @returns the command list (empty strings dropped)
 */
export function profileCommands(profile: Profile): string[] {
  const raw = Array.isArray(profile.command) ? profile.command : [profile.command];
  return raw.filter((cmd) => cmd.length > 0);
}

/**
 * Reads the theme, all-display brightness, and volume a profile applies.
 * Per-monitor brightness and other commands are ignored.
 * @param profile - profile to inspect
 * @returns the parsed settings; last matching command wins
 */
export function parseProfileSettings(profile: Profile): ProfileSettings {
  const settings: ProfileSettings = { theme: null, brightness: null, volume: null };
  for (const cmd of profileCommands(profile)) {
    const brightness = BRIGHTNESS_RE.exec(cmd);
    if (brightness) settings.brightness = Number(brightness[1]);
    const volume = VOLUME_RE.exec(cmd);
    if (volume) settings.volume = Number(volume[1]);
    const theme = THEME_RE.exec(cmd);
    if (theme) settings.theme = theme[1] as ProfileTheme;
  }
  return settings;
}

/**
 * Returns a copy of `profile` whose theme / brightness / volume commands reflect `settings`.
 * Unrelated commands (per-monitor brightness, tiling, wallpaper, ...) are preserved first;
 * a `null` setting drops that command entirely.
 * @param profile - profile to rewrite
 * @param settings - desired settings
 * @returns the updated profile
 */
export function applyProfileSettings(profile: Profile, settings: ProfileSettings): Profile {
  const kept = profileCommands(profile).filter(
    (cmd) => !BRIGHTNESS_RE.test(cmd) && !VOLUME_RE.test(cmd) && !THEME_RE.test(cmd),
  );
  if (settings.brightness !== null) kept.push(`command/changeBrightness/${settings.brightness}`);
  if (settings.theme !== null) kept.push(`command/changeDarkMode/${settings.theme}`);
  if (settings.volume !== null) kept.push(`command/changeVolume/${settings.volume}`);
  return { ...profile, command: kept };
}

/** Speaker kinds used to pick an icon for an audio output. */
export type SpeakerKind = 'headphones' | 'speaker';

const HEADPHONE_RE = /head[\s_-]?(phone|set)|ear[\s_-]?phone|bud/i;

/**
 * Classifies an audio output by its OS name: headphones/headsets/earbuds vs. everything else.
 * Case-insensitive; tolerates space, dash, or underscore separators ("Ear-Buds", "head phones").
 * @param originalName - native OS device name
 * @returns `'headphones'` for head/ear-worn devices, otherwise `'speaker'`
 */
export function speakerKind(originalName: string): SpeakerKind {
  return HEADPHONE_RE.test(originalName) ? 'headphones' : 'speaker';
}
