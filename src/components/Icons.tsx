import type { ReactNode } from 'react';

/** Icon slots used by the main popup. */
export type IconName =
  | 'allDisplays'
  | 'laptop'
  | 'monitor'
  | 'speaker'
  | 'speakerMedium'
  | 'speakerMuted'
  | 'headphones'
  | 'speakerDevice'
  | 'settings'
  | 'chevronDown'
  | 'chevronUp'
  | 'chevronRight';

/** Semi-transparent fill for the duotone body layer; stroke stays full strength. */
const TINT = { fill: 'currentColor', fillOpacity: 0.25 };

/** Speaker body shared by every volume state. */
const SPEAKER_BODY = <path d='M4 9h3l5-4v14l-5-4H4z' {...TINT} />;

/** Duotone glyphs (24x24 viewBox) drawn with `currentColor`. */
const PATHS: Record<IconName, ReactNode> = {
  allDisplays: (
    <>
      <rect x='1.5' y='7' width='13' height='9' rx='1.5' {...TINT} />
      <path d='M5 7V5.5A1.5 1.5 0 0 1 6.5 4h14A1.5 1.5 0 0 1 22 5.5v8a1.5 1.5 0 0 1-1.5 1.5H14.5' />
      <path d='M8 16v3M5 20h6' />
    </>
  ),
  laptop: (
    <>
      <rect x='4' y='5' width='16' height='11' rx='1.5' {...TINT} />
      <path d='M2 19h20' />
    </>
  ),
  monitor: (
    <>
      <rect x='2.5' y='4' width='19' height='12' rx='1.5' {...TINT} />
      <path d='M12 16v4M8 20h8' />
    </>
  ),
  speaker: (
    <>
      {SPEAKER_BODY}
      <path d='M16 9a4 4 0 0 1 0 6M18.5 6.5a7.5 7.5 0 0 1 0 11' />
    </>
  ),
  speakerMedium: (
    <>
      {SPEAKER_BODY}
      <path d='M16 9a4 4 0 0 1 0 6' />
    </>
  ),
  speakerMuted: (
    <>
      {SPEAKER_BODY}
      <path d='M16 9.5l5 5M21 9.5l-5 5' />
    </>
  ),
  headphones: (
    <>
      <path d='M4 15v-3a8 8 0 0 1 16 0v3' />
      <rect x='3' y='14' width='4.5' height='6.5' rx='1.5' {...TINT} />
      <rect x='16.5' y='14' width='4.5' height='6.5' rx='1.5' {...TINT} />
    </>
  ),
  speakerDevice: (
    <>
      <rect x='5.5' y='2.5' width='13' height='19' rx='2' {...TINT} />
      <circle cx='12' cy='14.5' r='3.5' />
      <circle cx='12' cy='6.8' r='1.2' />
    </>
  ),
  chevronDown: <path d='M6 9l6 6 6-6' />,
  chevronUp: <path d='M6 15l6-6 6 6' />,
  chevronRight: <path d='M9 6l6 6-6 6' />,
  settings: (
    <>
      <path
        d='M10.3 2.8h3.4l.5 2.4 1.7 1 2.3-.8 1.7 2.9-1.8 1.6v2l1.8 1.6-1.7 2.9-2.3-.8-1.7 1-.5 2.4h-3.4l-.5-2.4-1.7-1-2.3.8-1.7-2.9 1.8-1.6v-2L3.9 8.3l1.7-2.9 2.3.8 1.7-1z'
        {...TINT}
      />
      <circle cx='12' cy='12' r='3' />
    </>
  ),
};

/**
 * Renders a popup icon as inline duotone SVG. Colors follow `currentColor`,
 * so the icon adapts to dark and light themes automatically.
 * @param name - icon slot to draw
 * @param size - width/height in CSS px (default 20)
 * @param strokeWidth - line weight in viewBox units (default 1.9)
 */
export function Icon({
  name,
  size = 20,
  strokeWidth = 1.9,
}: {
  name: IconName;
  size?: number;
  strokeWidth?: number;
}) {
  return (
    <svg
      data-icon={name}
      width={size}
      height={size}
      viewBox='0 0 24 24'
      aria-hidden='true'
      fill='none'
      stroke='currentColor'
      strokeWidth={strokeWidth}
      strokeLinecap='round'
      strokeLinejoin='round'>
      {PATHS[name]}
    </svg>
  );
}
