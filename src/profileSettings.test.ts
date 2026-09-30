import { describe, it, expect } from 'vitest';
import { applyProfileSettings, parseProfileSettings, speakerKind } from './profileSettings';

describe('parseProfileSettings', () => {
  /** Reads all-display brightness, theme, and volume; ignores per-monitor brightness. */
  it('reads theme, brightness, and volume commands', () => {
    expect(
      parseProfileSettings({
        name: 'Focus',
        command: [
          'command/changeBrightness/2/40',
          'command/changeBrightness/75',
          'command/changeDarkMode/dark',
          'command/changeVolume/30',
        ],
      }),
    ).toEqual({ theme: 'dark', brightness: 75, volume: 30 });
  });

  /** A single-string command with nothing recognized yields all nulls. */
  it('returns nulls when nothing matches', () => {
    expect(parseProfileSettings({ name: 'x', command: 'command/tile/maximize' })).toEqual({
      theme: null,
      brightness: null,
      volume: null,
    });
  });
});

describe('applyProfileSettings', () => {
  /** Unrelated commands survive first; managed commands are replaced in fixed order. */
  it('replaces managed commands and keeps others', () => {
    const out = applyProfileSettings(
      { name: 'Focus', command: ['command/changeVolume/10', 'command/tile/maximize'] },
      { theme: 'light', brightness: 60, volume: null },
    );
    expect(out.command).toEqual([
      'command/tile/maximize',
      'command/changeBrightness/60',
      'command/changeDarkMode/light',
    ]);
  });
});

describe('speakerKind', () => {
  it.each([
    ['AirPods Pro Headphones', 'headphones'],
    ['head-phones', 'headphones'],
    ['Head Phone', 'headphones'],
    ['Galaxy Buds2 Pro', 'headphones'],
    ['Ear_Bud Left', 'headphones'],
    ['USB Headset', 'headphones'],
    ['MacBook Pro Speakers', 'speaker'],
    ['DELL U2723QE', 'speaker'],
  ])('classifies %s as %s', (name, kind) => {
    expect(speakerKind(name)).toBe(kind);
  });
});
