import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import SettingsPanel from './SettingsPanel';

const mockInvoke = vi.mocked(invoke);

/** Build a complete Preferences object with reasonable defaults so tests can
 * override only what they care about. */
function buildPrefs(overrides: Record<string, unknown> = {}) {
  return {
    showIndividualDisplays: false,
    minBrightness: 10,
    keyBindings: [],
    profiles: [],
    nightModeSchedule: {
      enabled: false,
      nightStart: '21:00',
      nightBrightness: 20,
      dayStart: '07:00',
      dayBrightness: 100,
      nightCommands: [],
      dayCommands: [],
    },
    showContrast: false,
    debugLogging: false,
    launchAtLogin: false,
    monitorConfigs: [],
    tiling: {
      enabled: true,
      halfRatio: 50,
      thirdRatio: 33,
      gap: 0,
      tileSnapEnabled: true,
      sideEdgeTrigger: 18,
      topEdgeTrigger: 18,
      cornerTrigger: 30,
      exposeEnabled: true,
      exposeColumns: 3,
      exposeRows: 3,
      exposeLayoutStrategy: 'spread',
      exposeMinWidth: 400,
      exposeMinHeight: 300,
    },
    layoutPresets: [],
    wallpaper: {
      fit: 'fill',
      currentWallpaperPath: null,
      perMonitorWallpapers: [],
      slideshowEnabled: false,
      slideshowFolder: null,
      slideshowIntervalMinutes: 30,
      slideshowOrder: 'forward',
    },
    ...overrides,
  };
}

/** Wire invoke() mock for SettingsPanel: preferences, tiling, accessibility. */
function setupInvoke(opts: {
  prefs?: ReturnType<typeof buildPrefs>;
  tilingSupported?: boolean;
  accessibilityTrusted?: boolean;
  prefsRejects?: boolean;
  tilingRejects?: boolean;
  accessRejects?: boolean;
}) {
  mockInvoke.mockReset();
  mockInvoke.mockImplementation((cmd: string) => {
    if (cmd === 'get_preferences') {
      if (opts.prefsRejects) return Promise.reject(new Error('boom'));
      return Promise.resolve(opts.prefs ?? buildPrefs());
    }
    if (cmd === 'get_tiling_supported') {
      if (opts.tilingRejects) return Promise.reject(new Error('boom'));
      return Promise.resolve(opts.tilingSupported ?? true);
    }
    if (cmd === 'get_accessibility_trusted') {
      if (opts.accessRejects) return Promise.reject(new Error('boom'));
      return Promise.resolve(opts.accessibilityTrusted ?? true);
    }
    if (cmd === 'save_preferences') return Promise.resolve(undefined);
    if (cmd === 'open_accessibility_settings') return Promise.resolve(undefined);
    return Promise.resolve(undefined);
  });
}

beforeEach(() => {
  vi.spyOn(console, 'error').mockImplementation(() => {});
});

afterEach(() => {
  vi.restoreAllMocks();
});

/** Wait long enough for the SettingsPanel debounce (300ms) to flush + save. */
async function waitForSave() {
  await waitFor(
    () => {
      expect(mockInvoke).toHaveBeenCalledWith('save_preferences', expect.any(Object));
    },
    { timeout: 2000 },
  );
}

describe('SettingsPanel', () => {
  it('renders null until preferences load', () => {
    setupInvoke({});
    const { container } = render(
      <SettingsPanel onClose={() => {}} onPreferencesSaved={() => {}} />,
    );
    expect(container.firstChild).toBeNull();
  });

  it('renders Settings title and tabs when tiling is supported', async () => {
    setupInvoke({ tilingSupported: true });
    render(<SettingsPanel onClose={() => {}} onPreferencesSaved={() => {}} />);

    await waitFor(() => {
      expect(screen.getByRole('button', { name: 'Tiling' })).toBeInTheDocument();
    });
    expect(screen.getByRole('button', { name: 'General' })).toBeInTheDocument();
    expect(screen.getByText('Settings')).toBeInTheDocument();
  });

  it('hides tabs when tiling is not supported', async () => {
    setupInvoke({ tilingSupported: false });
    render(<SettingsPanel onClose={() => {}} onPreferencesSaved={() => {}} />);
    await waitFor(() => {
      expect(screen.getByText('Settings')).toBeInTheDocument();
    });
    expect(screen.queryByRole('button', { name: 'Tiling' })).not.toBeInTheDocument();
  });

  it('calls onClose when the close button is clicked', async () => {
    setupInvoke({});
    const onClose = vi.fn();
    const user = userEvent.setup();
    render(<SettingsPanel onClose={onClose} onPreferencesSaved={() => {}} />);
    await waitFor(() => expect(screen.getByText('Settings')).toBeInTheDocument());
    await user.click(screen.getByTitle('Close'));
    expect(onClose).toHaveBeenCalledOnce();
  });

  it('clamps minBrightness below 5 up to 5 on load', async () => {
    setupInvoke({ prefs: buildPrefs({ minBrightness: 1 }) });
    render(<SettingsPanel onClose={() => {}} onPreferencesSaved={() => {}} />);
    await waitFor(() => {
      const sliders = screen.getAllByRole('slider');
      expect(sliders[0]).toHaveValue('5');
    });
  });

  it('toggles "Show Contrast Slider" and auto-saves', async () => {
    setupInvoke({});
    const onPreferencesSaved = vi.fn();
    const user = userEvent.setup();
    render(<SettingsPanel onClose={() => {}} onPreferencesSaved={onPreferencesSaved} />);
    await waitFor(() => expect(screen.getByText('Settings')).toBeInTheDocument());

    await user.click(screen.getByLabelText('Show Contrast Slider'));
    await waitForSave();
    await waitFor(() => expect(onPreferencesSaved).toHaveBeenCalled());

    expect(mockInvoke).toHaveBeenCalledWith(
      'save_preferences',
      expect.objectContaining({
        preferences: expect.objectContaining({ showContrast: true }),
      }),
    );
  });

  it('toggles "Launch at Login" and persists', async () => {
    setupInvoke({});
    const user = userEvent.setup();
    render(<SettingsPanel onClose={() => {}} onPreferencesSaved={() => {}} />);
    await waitFor(() => expect(screen.getByText('Settings')).toBeInTheDocument());

    await user.click(screen.getByLabelText('Launch at Login'));
    await waitForSave();
    expect(mockInvoke).toHaveBeenCalledWith(
      'save_preferences',
      expect.objectContaining({
        preferences: expect.objectContaining({ launchAtLogin: true }),
      }),
    );
  });

  it('enables Night Mode Schedule and reveals night/day sections', async () => {
    setupInvoke({});
    const user = userEvent.setup();
    render(<SettingsPanel onClose={() => {}} onPreferencesSaved={() => {}} />);
    await waitFor(() => expect(screen.getByText('Settings')).toBeInTheDocument());

    expect(screen.queryByText('Night')).not.toBeInTheDocument();
    await user.click(screen.getByLabelText('Night Mode Schedule'));
    expect(screen.getByText('Night')).toBeInTheDocument();
    expect(screen.getByText('Day')).toBeInTheDocument();

    await waitForSave();
  });

  it('changes wallpaper fit and auto-saves', async () => {
    setupInvoke({});
    const user = userEvent.setup();
    render(<SettingsPanel onClose={() => {}} onPreferencesSaved={() => {}} />);
    await waitFor(() => expect(screen.getByText('Wallpaper Fit')).toBeInTheDocument());

    const fitSelect = screen.getByDisplayValue('Fill Screen') as HTMLSelectElement;
    await user.selectOptions(fitSelect, 'stretch');
    await waitForSave();

    expect(mockInvoke).toHaveBeenCalledWith(
      'save_preferences',
      expect.objectContaining({
        preferences: expect.objectContaining({
          wallpaper: expect.objectContaining({ fit: 'stretch' }),
        }),
      }),
    );
  });

  it('enables wallpaper slideshow and reveals folder/interval/order controls', async () => {
    setupInvoke({});
    const user = userEvent.setup();
    render(<SettingsPanel onClose={() => {}} onPreferencesSaved={() => {}} />);
    await waitFor(() => expect(screen.getByText('Settings')).toBeInTheDocument());

    await user.click(screen.getByLabelText('Enable Wallpaper Slideshow'));
    expect(screen.getByText('Slideshow Folder')).toBeInTheDocument();
    expect(screen.getByText('Interval')).toBeInTheDocument();
    expect(screen.getByText('Slideshow Order')).toBeInTheDocument();
  });

  it('renders monitor rows with reorder buttons and visibility toggle', async () => {
    setupInvoke({
      prefs: buildPrefs({
        monitorConfigs: [
          {
            uid: '1::Dell',
            apiId: '1',
            apiName: 'Dell U2723QE',
            label: '',
            sortOrder: 0,
            hidden: false,
            brightnessMode: 'auto',
          },
          {
            uid: '2::LG',
            apiId: '2',
            apiName: 'LG 27UK850',
            label: 'My LG',
            sortOrder: 1,
            hidden: false,
            brightnessMode: 'auto',
          },
          {
            uid: '0::Built',
            apiId: 'builtin',
            apiName: 'Built-in Display',
            label: '',
            sortOrder: 2,
            hidden: false,
            brightnessMode: 'auto',
          },
        ],
      }),
    });
    const user = userEvent.setup();
    render(<SettingsPanel onClose={() => {}} onPreferencesSaved={() => {}} />);

    await waitFor(() => expect(screen.getByText('Dell U2723QE')).toBeInTheDocument());

    // Built-in monitors do NOT show the Hide/Show button.
    const hideButtons = screen.getAllByText(/^Hide$/);
    expect(hideButtons).toHaveLength(2);

    await user.click(hideButtons[0]);
    await waitForSave();
    expect(mockInvoke).toHaveBeenCalledWith(
      'save_preferences',
      expect.objectContaining({
        preferences: expect.objectContaining({
          monitorConfigs: expect.arrayContaining([
            expect.objectContaining({ uid: '1::Dell', hidden: true }),
          ]),
        }),
      }),
    );
  });

  it('changes a monitor brightnessMode via the dropdown and persists it', async () => {
    setupInvoke({
      prefs: buildPrefs({
        monitorConfigs: [
          {
            uid: '1::Samsung',
            apiId: '1',
            apiName: 'Samsung Smart Monitor',
            label: '',
            sortOrder: 0,
            hidden: false,
            brightnessMode: 'auto',
          },
        ],
      }),
    });
    const user = userEvent.setup();
    render(<SettingsPanel onClose={() => {}} onPreferencesSaved={() => {}} />);
    await waitFor(() => expect(screen.getByText('Samsung Smart Monitor')).toBeInTheDocument());

    // The dropdown defaults to "Auto" for an external monitor in auto mode.
    const modeSelect = screen.getByDisplayValue('Auto') as HTMLSelectElement;
    expect(modeSelect).toBeInTheDocument();

    await user.selectOptions(modeSelect, 'overlay');
    await waitForSave();

    expect(mockInvoke).toHaveBeenCalledWith(
      'save_preferences',
      expect.objectContaining({
        preferences: expect.objectContaining({
          monitorConfigs: expect.arrayContaining([
            expect.objectContaining({
              uid: '1::Samsung',
              brightnessMode: 'overlay',
            }),
          ]),
        }),
      }),
    );
  });

  it('does not render the brightnessMode dropdown for built-in monitors', async () => {
    setupInvoke({
      prefs: buildPrefs({
        monitorConfigs: [
          {
            uid: '0::Built',
            apiId: 'builtin',
            apiName: 'Built-in Display',
            label: '',
            sortOrder: 0,
            hidden: false,
            brightnessMode: 'auto',
          },
        ],
      }),
    });
    render(<SettingsPanel onClose={() => {}} onPreferencesSaved={() => {}} />);
    await waitFor(() => expect(screen.getByText('Built-in Display')).toBeInTheDocument());

    // No dropdown for built-in (it has its own native brightness path).
    expect(screen.queryByDisplayValue('Auto')).not.toBeInTheDocument();
  });

  it('swaps monitors via up/down buttons', async () => {
    setupInvoke({
      prefs: buildPrefs({
        monitorConfigs: [
          {
            uid: '1::Dell',
            apiId: '1',
            apiName: 'Dell',
            label: '',
            sortOrder: 0,
            hidden: false,
            brightnessMode: 'auto',
          },
          {
            uid: '2::LG',
            apiId: '2',
            apiName: 'LG',
            label: '',
            sortOrder: 1,
            hidden: false,
            brightnessMode: 'auto',
          },
        ],
      }),
    });
    const user = userEvent.setup();
    render(<SettingsPanel onClose={() => {}} onPreferencesSaved={() => {}} />);
    await waitFor(() => expect(screen.getByText('Dell')).toBeInTheDocument());

    const moveDowns = screen.getAllByTitle('Move down');
    const moveUps = screen.getAllByTitle('Move up');
    expect(moveUps[0]).toBeDisabled();
    expect(moveDowns[moveDowns.length - 1]).toBeDisabled();

    await user.click(moveDowns[0]);
    await waitForSave();
  });

  it('enters and commits monitor label edit mode on Enter', async () => {
    setupInvoke({
      prefs: buildPrefs({
        monitorConfigs: [
          {
            uid: '1::Dell',
            apiId: '1',
            apiName: 'Dell U2723QE',
            label: '',
            sortOrder: 0,
            hidden: false,
            brightnessMode: 'auto',
          },
        ],
      }),
    });
    const user = userEvent.setup();
    render(<SettingsPanel onClose={() => {}} onPreferencesSaved={() => {}} />);
    await waitFor(() => expect(screen.getByText('Dell U2723QE')).toBeInTheDocument());

    await user.click(screen.getByText('Dell U2723QE'));
    const input = screen.getByRole('textbox');
    await user.clear(input);
    await user.type(input, 'My Display{Enter}');
    await waitForSave();
    expect(mockInvoke).toHaveBeenCalledWith(
      'save_preferences',
      expect.objectContaining({
        preferences: expect.objectContaining({
          monitorConfigs: expect.arrayContaining([
            expect.objectContaining({ uid: '1::Dell', label: 'My Display' }),
          ]),
        }),
      }),
    );
  });

  it('cancels monitor label edit on Escape', async () => {
    setupInvoke({
      prefs: buildPrefs({
        monitorConfigs: [
          {
            uid: '1::Dell',
            apiId: '1',
            apiName: 'Dell',
            label: '',
            sortOrder: 0,
            hidden: false,
            brightnessMode: 'auto',
          },
        ],
      }),
    });
    const user = userEvent.setup();
    render(<SettingsPanel onClose={() => {}} onPreferencesSaved={() => {}} />);
    await waitFor(() => expect(screen.getByText('Dell')).toBeInTheDocument());

    await user.click(screen.getByText('Dell'));
    const input = screen.getByRole('textbox');
    await user.type(input, 'Bogus{Escape}');
    expect(screen.queryByRole('textbox')).not.toBeInTheDocument();
  });

  it('switches to Tiling tab and shows tiling controls', async () => {
    setupInvoke({});
    const user = userEvent.setup();
    render(<SettingsPanel onClose={() => {}} onPreferencesSaved={() => {}} />);
    await waitFor(() => expect(screen.getByRole('button', { name: 'Tiling' })).toBeInTheDocument());

    await user.click(screen.getByRole('button', { name: 'Tiling' }));
    expect(screen.getByLabelText('Enable Window Tiling')).toBeInTheDocument();
    expect(screen.getByLabelText('Enable Tile Snap (drag to edge)')).toBeInTheDocument();
    expect(screen.getByLabelText('Enable Exposé')).toBeInTheDocument();
  });

  it('shows accessibility warning when tile snap is on but not trusted', async () => {
    setupInvoke({ accessibilityTrusted: false });
    const user = userEvent.setup();
    render(<SettingsPanel onClose={() => {}} onPreferencesSaved={() => {}} />);
    await waitFor(() => expect(screen.getByRole('button', { name: 'Tiling' })).toBeInTheDocument());

    await user.click(screen.getByRole('button', { name: 'Tiling' }));
    expect(screen.getByText(/Accessibility permission required/)).toBeInTheDocument();

    await user.click(screen.getByText('Open Accessibility Settings'));
    expect(mockInvoke).toHaveBeenCalledWith('open_accessibility_settings');
  });

  it('hides tiling sub-controls when tiling is disabled', async () => {
    setupInvoke({
      prefs: buildPrefs({
        tiling: {
          enabled: false,
          halfRatio: 50,
          thirdRatio: 33,
          gap: 0,
          tileSnapEnabled: true,
          sideEdgeTrigger: 18,
          topEdgeTrigger: 18,
          cornerTrigger: 30,
          exposeEnabled: true,
          exposeColumns: 3,
          exposeRows: 3,
          exposeLayoutStrategy: 'spread',
          exposeMinWidth: 400,
          exposeMinHeight: 300,
        },
      }),
    });
    const user = userEvent.setup();
    render(<SettingsPanel onClose={() => {}} onPreferencesSaved={() => {}} />);
    await waitFor(() => expect(screen.getByRole('button', { name: 'Tiling' })).toBeInTheDocument());

    await user.click(screen.getByRole('button', { name: 'Tiling' }));
    expect(screen.getByLabelText('Enable Window Tiling')).toBeInTheDocument();
    expect(screen.queryByLabelText('Enable Tile Snap (drag to edge)')).not.toBeInTheDocument();
    expect(screen.queryByLabelText('Enable Exposé')).not.toBeInTheDocument();
  });

  it('hides expose sub-controls when expose is disabled', async () => {
    setupInvoke({
      prefs: buildPrefs({
        tiling: {
          enabled: true,
          halfRatio: 50,
          thirdRatio: 33,
          gap: 0,
          tileSnapEnabled: false,
          sideEdgeTrigger: 18,
          topEdgeTrigger: 18,
          cornerTrigger: 30,
          exposeEnabled: false,
          exposeColumns: 3,
          exposeRows: 3,
          exposeLayoutStrategy: 'spread',
          exposeMinWidth: 400,
          exposeMinHeight: 300,
        },
      }),
    });
    const user = userEvent.setup();
    render(<SettingsPanel onClose={() => {}} onPreferencesSaved={() => {}} />);
    await waitFor(() => expect(screen.getByRole('button', { name: 'Tiling' })).toBeInTheDocument());

    await user.click(screen.getByRole('button', { name: 'Tiling' }));
    expect(screen.queryByText('Exposé Grid Size')).not.toBeInTheDocument();
    expect(screen.queryByText('Snap Zones')).not.toBeInTheDocument();
  });

  it('changes expose layout strategy via dropdown', async () => {
    setupInvoke({});
    const user = userEvent.setup();
    render(<SettingsPanel onClose={() => {}} onPreferencesSaved={() => {}} />);
    await waitFor(() => expect(screen.getByRole('button', { name: 'Tiling' })).toBeInTheDocument());

    await user.click(screen.getByRole('button', { name: 'Tiling' }));
    const strategySelect = screen.getByDisplayValue(/Spread/) as HTMLSelectElement;
    await user.selectOptions(strategySelect, 'fill');
    await waitForSave();

    expect(mockInvoke).toHaveBeenCalledWith(
      'save_preferences',
      expect.objectContaining({
        preferences: expect.objectContaining({
          tiling: expect.objectContaining({ exposeLayoutStrategy: 'fill' }),
        }),
      }),
    );
  });

  it('falls back to defaults when get_tiling_supported / get_accessibility_trusted reject', async () => {
    setupInvoke({ tilingRejects: true, accessRejects: true });
    render(<SettingsPanel onClose={() => {}} onPreferencesSaved={() => {}} />);
    await waitFor(() => expect(screen.getByText('Settings')).toBeInTheDocument());
    expect(screen.queryByRole('button', { name: 'Tiling' })).not.toBeInTheDocument();
  });

  it('handles get_preferences rejection by remaining null (no crash)', async () => {
    setupInvoke({ prefsRejects: true });
    const { container } = render(
      <SettingsPanel onClose={() => {}} onPreferencesSaved={() => {}} />,
    );
    await waitFor(() => {
      // Ensure invoke has been called and the rejection settled.
      expect(mockInvoke).toHaveBeenCalledWith('get_preferences');
    });
    expect(container.firstChild).toBeNull();
  });
});
