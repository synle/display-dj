import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import DpiSettings, { validateDpiRange } from './DpiSettings';

const mockInvoke = vi.mocked(invoke);

const DISPLAYS = [
  { id: '1', name: 'Built-in Retina Display', current: 200, options: [100, 150, 200, 231] },
  { id: '3', name: 'TYPEC', current: 100, options: [100, 150, 240] },
];

/** Renders DpiSettings with spies and sensible defaults. */
function setup(enabled = true, min = 60, max = 200) {
  const onEnabledChange = vi.fn();
  const onRangeChange = vi.fn();
  render(
    <DpiSettings
      enabled={enabled}
      minPercent={min}
      maxPercent={max}
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
  /** Accepts integers inside 60-250 with min <= max. */
  it('accepts a valid range', () => {
    expect(validateDpiRange('75', '200')).toEqual({ min: 75, max: 200 });
  });

  /** Rejects non-integers. */
  it('rejects non-integer input', () => {
    expect(validateDpiRange('7.5', '200')).toEqual({ error: 'Min and max must be whole numbers.' });
  });

  /** Rejects values outside the absolute caps. */
  it('rejects values outside 60-250', () => {
    expect(validateDpiRange('59', '200')).toEqual({
      error: 'Values must be between 60% and 250%.',
    });
    expect(validateDpiRange('60', '251')).toEqual({
      error: 'Values must be between 60% and 250%.',
    });
  });

  /** Rejects min above max. */
  it('rejects inverted range', () => {
    expect(validateDpiRange('150', '100')).toEqual({ error: 'Min must not exceed max.' });
  });
});

describe('DpiSettings', () => {
  /** Disabled: only the beta checkbox, no backend call. */
  it('shows only the checkbox when disabled', () => {
    const { onEnabledChange } = setup(false);
    expect(screen.getByText('Show DPI Settings (Beta)')).toBeInTheDocument();
    expect(screen.queryByText('DPI Settings (Beta)')).not.toBeInTheDocument();
    expect(mockInvoke).not.toHaveBeenCalled();
    screen.getByRole('checkbox').click();
    expect(onEnabledChange).toHaveBeenCalledWith(true);
  });

  /** Enabled: lists displays with options filtered to the band. */
  it('lists displays with band-filtered options', async () => {
    setup(true, 60, 200);
    const builtIn = await screen.findByLabelText('DPI scale for Built-in Retina Display');
    expect(screen.getByText(/Beta: changes your OS display scaling/)).toBeInTheDocument();
    expect(Array.from((builtIn as HTMLSelectElement).options).map((o) => o.value)).toEqual([
      '100',
      '150',
      '200',
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
    const min = screen.getByLabelText('Min DPI percent');
    await user.clear(min);
    await user.type(min, '40{Enter}');
    expect(screen.getByText('Values must be between 60% and 250%.')).toBeInTheDocument();
    expect(onRangeChange).not.toHaveBeenCalled();
  });

  /** Valid range commits the parsed pair. */
  it('commits a valid range', async () => {
    const user = userEvent.setup();
    const { onRangeChange } = setup();
    const max = screen.getByLabelText('Max DPI percent');
    await user.clear(max);
    await user.type(max, '250{Enter}');
    expect(onRangeChange).toHaveBeenCalledWith(60, 250);
  });

  /** Backend failure (e.g. Wayland) surfaces its message. */
  it('shows backend error', async () => {
    mockInvoke.mockRejectedValue('DPI scaling requires an X11 session');
    setup();
    expect(await screen.findByText('DPI scaling requires an X11 session')).toBeInTheDocument();
  });
});
