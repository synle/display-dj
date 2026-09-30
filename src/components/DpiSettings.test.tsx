import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import {
  DpiOptions,
  DpiScaleDropdown,
  dpiOptionsFor,
  useDpiDisplays,
  validateDpiRange,
} from './DpiSettings';

const mockInvoke = vi.mocked(invoke);

const DISPLAYS = [
  {
    id: '1',
    name: 'Built-in Retina Display',
    current: 200,
    options: [100, 150, 200, 231],
    continuous: false,
  },
  { id: '3', name: 'TYPEC', current: 100, options: [100, 150, 240], continuous: false },
];

/** Test harness composing the DPI pieces the way SettingsPanel does. */
function Harness(props: {
  enabled: boolean;
  min: number;
  max: number;
  onEnabledChange: (v: boolean) => void;
  onRangeChange: (min: number, max: number, step: number) => void;
}) {
  const dpi = useDpiDisplays(props.enabled);
  return (
    <>
      <DpiOptions
        enabled={props.enabled}
        minPercent={props.min}
        maxPercent={props.max}
        stepPercent={5}
        showStep={false}
        onEnabledChange={props.onEnabledChange}
        onRangeChange={props.onRangeChange}
      />
      {dpi.error && <div>{dpi.error}</div>}
      {props.enabled &&
        dpi.displays.map((d) => (
          <DpiScaleDropdown
            key={d.id}
            display={d}
            minPercent={props.min}
            maxPercent={props.max}
            stepPercent={5}
            disabled={false}
            onApply={(id, p) => void dpi.apply(id, p)}
          />
        ))}
    </>
  );
}

/** Renders the DPI pieces with spies and sensible defaults. */
function setup(enabled = true, min = 60, max = 200) {
  const onEnabledChange = vi.fn();
  const onRangeChange = vi.fn();
  render(
    <Harness
      enabled={enabled}
      min={min}
      max={max}
      onEnabledChange={onEnabledChange}
      onRangeChange={onRangeChange}
    />,
  );
  return { onEnabledChange, onRangeChange };
}

beforeEach(() => {
  mockInvoke.mockReset();
  mockInvoke.mockImplementation(async (cmd: string) =>
    cmd === 'get_dpi_displays' ? DISPLAYS : undefined,
  );
});

describe('validateDpiRange', () => {
  /** Accepts integers inside 50-500 with min <= max. */
  it('accepts a valid range', () => {
    expect(validateDpiRange('75', '200')).toEqual({ min: 75, max: 200, step: 5 });
  });

  /** Rejects non-integers. */
  it('rejects non-integer input', () => {
    expect(validateDpiRange('7.5', '200')).toEqual({
      error: 'Min, max, and step must be whole numbers.',
    });
  });

  /** Rejects values outside the absolute caps. */
  it('rejects values outside 50-500', () => {
    expect(validateDpiRange('49', '200')).toEqual({
      error: 'Values must be between 50% and 500%.',
    });
    expect(validateDpiRange('60', '501')).toEqual({
      error: 'Values must be between 50% and 500%.',
    });
  });

  /** Rejects min above max. */
  it('rejects inverted range', () => {
    expect(validateDpiRange('150', '100')).toEqual({ error: 'Min must not exceed max.' });
  });
});

describe('dpiOptionsFor', () => {
  /** Discrete (macOS/Windows) lists pass through untouched by the band. */
  it('returns the OS list for discrete displays', () => {
    const d = {
      id: '1',
      name: 'X',
      current: 200,
      options: [100, 116, 200, 400],
      continuous: false,
    };
    expect(dpiOptionsFor(d, 150, 180, 5)).toEqual([100, 116, 200, 400]);
  });

  /** Continuous (Linux) builds a min..max grid at step, keeping current. */
  it('builds a stepped grid for continuous displays', () => {
    const d = { id: 'HDMI-1', name: 'HDMI-1', current: 133, options: [], continuous: true };
    expect(dpiOptionsFor(d, 60, 80, 10)).toEqual([60, 70, 80, 133]);
  });
});

describe('DPI settings controls', () => {
  /** Scale dropdown carries a tooltip explaining bigger vs smaller scale. */
  it('explains scaling in the dropdown tooltip', async () => {
    setup(true);
    await screen.findByLabelText('DPI scale for TYPEC');
    expect(screen.getAllByText(/Higher % = bigger text/).length).toBe(2);
  });

  /** Beta chip shows; the warning lives in the tooltip, band inputs are hidden. */
  it('shows beta chip, tooltip warning, and hides band inputs', () => {
    setup(true);
    expect(screen.getByText('beta')).toHaveClass('beta-chip');
    expect(screen.getByText(/Beta: changes OS scaling/)).toHaveClass('settings-dpi-beta');
    expect(screen.getByLabelText('Min % DPI percent').closest('.settings-dpi-range')).toHaveStyle({
      display: 'none',
    });
  });

  /** Disabled: only the beta checkbox, no backend call. */
  it('shows only the checkbox when disabled', () => {
    const { onEnabledChange } = setup(false);
    expect(screen.getByText('Show DPI Settings')).toBeInTheDocument();
    expect(screen.queryByLabelText('Min % DPI percent')).not.toBeInTheDocument();
    expect(mockInvoke).not.toHaveBeenCalled();
    screen.getByRole('checkbox').click();
    expect(onEnabledChange).toHaveBeenCalledWith(true);
  });

  /** Enabled: discrete displays list every OS option, ignoring the band. */
  it('lists every OS option for discrete displays', async () => {
    setup(true, 60, 200);
    const builtIn = await screen.findByLabelText('DPI scale for Built-in Retina Display');
    expect(Array.from((builtIn as HTMLSelectElement).options).map((o) => o.value)).toEqual([
      '100',
      '150',
      '200',
      '231',
    ]);
    expect((builtIn as HTMLSelectElement).value).toBe('200');
  });

  /** Selecting a value calls set_display_dpi with id and percent. */
  it('applies the selected scale', async () => {
    const user = userEvent.setup();
    setup();
    const typec = await screen.findByLabelText('DPI scale for TYPEC');
    await user.selectOptions(typec, '150');
    await waitFor(() =>
      expect(mockInvoke).toHaveBeenCalledWith('set_display_dpi', { id: '3', percent: 150 }),
    );
  });

  /** Invalid range shows an error and does not commit. */
  it('shows validation error and skips commit on invalid range', async () => {
    const user = userEvent.setup();
    const { onRangeChange } = setup();
    const min = screen.getByLabelText('Min % DPI percent');
    await user.clear(min);
    await user.type(min, '40{Enter}');
    expect(screen.getByText('Values must be between 50% and 500%.')).toBeInTheDocument();
    expect(onRangeChange).not.toHaveBeenCalled();
  });

  /** Valid range commits the parsed pair. */
  it('commits a valid range', async () => {
    const user = userEvent.setup();
    const { onRangeChange } = setup();
    const max = screen.getByLabelText('Max % DPI percent');
    await user.clear(max);
    await user.type(max, '250{Enter}');
    expect(onRangeChange).toHaveBeenCalledWith(60, 250, 5);
  });

  /** Backend failure (e.g. Wayland) surfaces its message. */
  it('shows backend error', async () => {
    mockInvoke.mockRejectedValue('DPI scaling requires an X11 session');
    setup();
    expect(await screen.findByText('DPI scaling requires an X11 session')).toBeInTheDocument();
  });
});
