import { render, screen, waitFor, act, fireEvent } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import App from './App';

const mockInvoke = vi.mocked(invoke);
const mockListen = vi.mocked(listen);

beforeEach(() => {
  mockInvoke.mockReset();
  mockListen.mockReset();
  mockListen.mockResolvedValue(() => {});
  mockInvoke.mockImplementation((cmd: string) => {
    switch (cmd) {
      case 'fetch_all_state':
        return Promise.resolve({
          monitors: [
            {
              id: 'builtin-0',
              uid: 'builtin-0::Built-in Display',
              name: 'Built-in Display',
              originalName: 'Built-in Display',
              brightness: 50,
              supportsBrightness: true,
              isBuiltIn: true,
            },
          ],
          isDark: false,
          volume: 50,
        });
      case 'get_monitors':
        return Promise.resolve([
          {
            id: 'builtin-0',
            uid: 'builtin-0::Built-in Display',
            name: 'Built-in Display',
            originalName: 'Built-in Display',
            brightness: 50,
            supportsBrightness: true,
            isBuiltIn: true,
          },
        ]);
      case 'get_dark_mode':
        return Promise.resolve(false);
      case 'get_volume':
        return Promise.resolve(50);
      case 'get_audio_output_devices':
        return Promise.resolve({
          devices: [
            {
              id: 'speakers',
              name: 'Desk Speakers',
              originalName: 'MacBook Pro Speakers',
              state: 'enabled',
              isBuiltIn: true,
            },
            {
              id: 'headphones',
              name: 'Headphones',
              originalName: 'USB Headphones',
              state: 'enabled',
              isBuiltIn: false,
            },
          ],
          selectedDeviceId: 'speakers',
        });
      case 'set_audio_output_device':
        return Promise.resolve({
          devices: [
            {
              id: 'speakers',
              name: 'Desk Speakers',
              originalName: 'MacBook Pro Speakers',
              state: 'enabled',
              isBuiltIn: true,
            },
            {
              id: 'headphones',
              name: 'Headphones',
              originalName: 'USB Headphones',
              state: 'enabled',
              isBuiltIn: false,
            },
          ],
          selectedDeviceId: 'headphones',
        });
      case 'set_audio_output_device_state':
        return Promise.resolve({
          devices: [
            {
              id: 'speakers',
              name: 'Desk Speakers',
              originalName: 'MacBook Pro Speakers',
              state: 'enabled',
              isBuiltIn: true,
            },
            {
              id: 'headphones',
              name: 'Headphones',
              originalName: 'USB Headphones',
              state: 'disabled',
              isBuiltIn: false,
            },
          ],
          selectedDeviceId: 'speakers',
        });
      case 'save_audio_output_order':
        return Promise.resolve({
          devices: [
            {
              id: 'headphones',
              name: 'Headphones',
              originalName: 'USB Headphones',
              state: 'enabled',
              isBuiltIn: false,
            },
            {
              id: 'speakers',
              name: 'Desk Speakers',
              originalName: 'MacBook Pro Speakers',
              state: 'enabled',
              isBuiltIn: true,
            },
          ],
          selectedDeviceId: 'speakers',
        });
      case 'get_preferences':
        return Promise.resolve({
          showIndividualDisplays: false,
          minBrightness: 10,
          keyBindings: [],
          profiles: [
            {
              name: 'Presentation',
              command: [
                'command/changeBrightness/100',
                'command/changeDarkMode/light',
                'command/changeVolume/50',
              ],
            },
            {
              name: 'Focus',
              command: [
                'command/changeBrightness/80',
                'command/changeDarkMode/dark',
                'command/changeVolume/30',
              ],
            },
            {
              name: 'Daylight',
              command: [
                'command/changeBrightness/100',
                'command/changeDarkMode/light',
                'command/changeVolume/100',
              ],
            },
          ],
          nightModeSchedule: {
            enabled: false,
            nightStart: '21:00',
            nightBrightness: 20,
            dayStart: '07:00',
            dayBrightness: 100,
          },
          debugLogging: false,
          launchAtLogin: false,
          monitorConfigs: [],
          audioOutputConfigs: [],
        });
      case 'get_app_version':
        return Promise.resolve('2.1.0');
      case 'get_keep_awake':
        return Promise.resolve(false);
      default:
        return Promise.resolve(undefined);
    }
  });
});

describe('App smoke test', () => {
  it('renders without crashing', () => {
    const { container } = render(<App />);
    expect(container.querySelector('.app')).toBeInTheDocument();
  });

  it('renders the header with title', () => {
    render(<App />);
    expect(screen.getByText('Display DJ')).toBeInTheDocument();
  });

  it('fetches initial data on mount', async () => {
    render(<App />);
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('fetch_all_state');
      expect(mockInvoke).toHaveBeenCalledWith('get_preferences');
      expect(mockInvoke).toHaveBeenCalledWith('get_keep_awake');
      expect(mockInvoke).toHaveBeenCalledWith('get_app_version');
    });
  });

  it('displays version from backend', async () => {
    render(<App />);
    await waitFor(() => {
      expect(screen.getByText('v2.1.0')).toBeInTheDocument();
    });
  });

  it('renders volume control', () => {
    render(<App />);
    const sliders = screen.getAllByRole('slider');
    expect(sliders.length).toBeGreaterThanOrEqual(1);
  });

  it('renders dark mode toggle', () => {
    render(<App />);
    expect(screen.getByText('DARK', { exact: false })).toBeInTheDocument();
    expect(screen.getByText('LIGHT', { exact: false })).toBeInTheDocument();
  });

  it('renders profile buttons', async () => {
    render(<App />);
    await waitFor(() => {
      expect(screen.getByText('Presentation')).toBeInTheDocument();
      expect(screen.getByText('Focus')).toBeInTheDocument();
      expect(screen.getByText('Daylight')).toBeInTheDocument();
    });
  });

  it('renders keep awake toggle', async () => {
    render(<App />);
    await waitFor(() => {
      expect(screen.getByText('Keep Awake: Off')).toBeInTheDocument();
    });
  });

  it('fetches keep awake state on mount', async () => {
    render(<App />);
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('get_keep_awake');
    });
  });

  it('shows all-monitors view by default (collapsed)', async () => {
    render(<App />);
    await waitFor(() => {
      expect(screen.getByText('All Monitors (1)')).toBeInTheDocument();
    });
  });

  it('shows the active audio output in collapsed mode without device rows', async () => {
    render(<App />);

    await waitFor(() => {
      expect(screen.getByText('All Speakers (2) - Desk Speakers')).toHaveClass('section-label');
    });
    expect(screen.queryByRole('radio')).not.toBeInTheDocument();
  });

  it('updates the active output when the tray selection event arrives', async () => {
    let audioOutputHandler:
      | ((event: {
          payload: {
            devices: Array<{
              id: string;
              name: string;
              originalName: string;
              state: 'enabled' | 'disabled' | 'hidden';
              isBuiltIn: boolean;
            }>;
            selectedDeviceId: string;
          };
        }) => void)
      | null = null;
    mockListen.mockImplementation((event: string, handler: (event: { payload: never }) => void) => {
      if (event === 'audio-output-changed') {
        audioOutputHandler = handler;
      }
      return Promise.resolve(() => {});
    });

    render(<App />);
    await screen.findByText('All Speakers (2) - Desk Speakers');

    act(() => {
      audioOutputHandler!({
        payload: {
          devices: [
            {
              id: 'headphones',
              name: 'Headphones',
              originalName: 'USB Headphones',
              state: 'enabled',
              isBuiltIn: false,
            },
          ],
          selectedDeviceId: 'headphones',
        },
      });
    });

    expect(screen.getByText('All Speakers (1) - Headphones')).toBeInTheDocument();
  });

  it('keeps the collapsed audio output heading readonly', async () => {
    const user = userEvent.setup();
    render(<App />);

    const heading = await screen.findByText('All Speakers (2) - Desk Speakers');
    await user.click(heading);

    expect(screen.queryByRole('textbox')).not.toBeInTheDocument();
    expect(mockInvoke).not.toHaveBeenCalledWith('rename_audio_output_device', expect.anything());
  });

  it('selects and renames audio outputs from expanded mode', async () => {
    const user = userEvent.setup();
    render(<App />);

    await waitFor(() => {
      expect(screen.getByText('All Monitors (1)')).toBeInTheDocument();
      expect(screen.getByText(/Desk Speakers/)).toBeInTheDocument();
    });
    await user.click(screen.getByTitle('Show output speakers'));
    await user.click(screen.getByRole('radio', { name: 'Select Headphones' }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('set_audio_output_device', {
        id: 'headphones',
      });
      expect(screen.getByRole('radio', { name: 'Select Headphones' })).toBeChecked();
      expect(screen.getByRole('radio', { name: 'Select Desk Speakers' })).not.toBeChecked();
    });

    await user.click(screen.getByTitle('Rename USB Headphones'));
    const input = screen.getByRole('textbox');
    await user.clear(input);
    await user.type(input, 'Studio Headphones{Enter}');

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('rename_audio_output_device', {
        id: 'headphones',
        label: 'Studio Headphones',
      });
      expect(screen.getByTitle('Rename USB Headphones')).toHaveTextContent('Studio Headphones');
    });
  });

  it('persists speaker ordering from expanded mode without state selectors', async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByTitle('Show output speakers'));
    expect(screen.queryByRole('combobox')).not.toBeInTheDocument();
    await user.click(screen.getByTitle('Move Headphones up'));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('save_audio_output_order', {
        orderedIds: ['headphones', 'speakers'],
      });
    });
  });

  it('polls audio outputs only while the visible main panel is active', async () => {
    vi.useFakeTimers();
    let visibility: DocumentVisibilityState = 'visible';
    const visibilitySpy = vi
      .spyOn(document, 'visibilityState', 'get')
      .mockImplementation(() => visibility);

    try {
      render(<App />);
      await act(async () => {
        await Promise.resolve();
        await Promise.resolve();
      });
      const initialCalls = mockInvoke.mock.calls.filter(
        ([command]) => command === 'get_audio_output_devices',
      ).length;
      expect(initialCalls).toBe(1);

      await act(async () => {
        vi.advanceTimersByTime(5_000);
        await Promise.resolve();
      });
      const polledCalls = mockInvoke.mock.calls.filter(
        ([command]) => command === 'get_audio_output_devices',
      ).length;
      expect(polledCalls).toBe(2);

      visibility = 'hidden';
      fireEvent(document, new Event('visibilitychange'));
      await act(async () => {
        vi.advanceTimersByTime(10_000);
        await Promise.resolve();
      });
      expect(
        mockInvoke.mock.calls.filter(([command]) => command === 'get_audio_output_devices'),
      ).toHaveLength(2);

      visibility = 'visible';
      fireEvent(document, new Event('visibilitychange'));
      await act(async () => {
        await Promise.resolve();
      });
      expect(
        mockInvoke.mock.calls.filter(([command]) => command === 'get_audio_output_devices'),
      ).toHaveLength(3);

      fireEvent.click(screen.getByTitle('Settings'));
      await act(async () => {
        vi.advanceTimersByTime(10_000);
        await Promise.resolve();
      });
      expect(
        mockInvoke.mock.calls.filter(([command]) => command === 'get_audio_output_devices'),
      ).toHaveLength(4);
    } finally {
      visibilitySpy.mockRestore();
      vi.useRealTimers();
    }
  });

  it('handles backend errors gracefully without crashing', async () => {
    mockInvoke.mockRejectedValue(new Error('backend unavailable'));
    const { container } = render(<App />);
    // App should still render even if all backend calls fail
    await waitFor(() => {
      expect(container.querySelector('.app')).toBeInTheDocument();
    });
  });

  it('renders collapsed and expanded views without JS errors', async () => {
    const errorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    const user = userEvent.setup();

    render(<App />);

    // Wait for initial data to load (collapsed view)
    await waitFor(() => {
      expect(screen.getByText('All Monitors (1)')).toBeInTheDocument();
    });

    // Expand output speakers; monitor controls stay collapsed.
    await user.click(screen.getByTitle('Show output speakers'));
    await waitFor(() => {
      expect(screen.getByRole('radio', { name: 'Select Desk Speakers' })).toBeChecked();
    });
    expect(screen.getByText('All Monitors (1)')).toBeInTheDocument();
    expect(screen.queryByText('Built-in Display')).not.toBeInTheDocument();

    // Expand monitor controls; output speakers stay expanded.
    await user.click(screen.getByTitle('Show individual monitors'));
    await waitFor(() => {
      expect(screen.getByText('Built-in Display')).toBeInTheDocument();
    });
    expect(screen.getByRole('radio', { name: 'Select Desk Speakers' })).toBeChecked();

    // Collapse output speakers; monitor controls stay expanded.
    await user.click(screen.getByTitle('Hide output speakers'));
    expect(screen.queryByRole('radio')).not.toBeInTheDocument();
    expect(screen.getByText('Built-in Display')).toBeInTheDocument();

    // Collapse monitor controls.
    await user.click(screen.getByTitle('Show all monitors control'));
    await waitFor(() => {
      expect(screen.getByText('All Monitors (1)')).toBeInTheDocument();
    });
    expect(screen.queryByRole('radio')).not.toBeInTheDocument();

    // No console.error calls should have occurred
    expect(errorSpy).not.toHaveBeenCalled();
    errorSpy.mockRestore();
  });

  it('renders with multiple monitors without JS errors', async () => {
    const errorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    const multiMonitors = [
      {
        id: 'builtin-0',
        uid: 'builtin-0::Built-in Display',
        name: 'Built-in Display',
        originalName: 'Built-in Display',
        brightness: 100,
        supportsBrightness: true,
        isBuiltIn: true,
      },
      {
        id: '1',
        uid: '1::Dell U2723QE',
        name: 'Dell U2723QE',
        originalName: 'Dell U2723QE',
        brightness: 80,
        supportsBrightness: true,
        isBuiltIn: false,
      },
      {
        id: '2',
        uid: '2::LG 27UK850',
        name: '',
        originalName: 'LG 27UK850',
        brightness: 60,
        supportsBrightness: true,
        isBuiltIn: false,
      },
    ];
    mockInvoke.mockImplementation((cmd: string) => {
      switch (cmd) {
        case 'fetch_all_state':
          return Promise.resolve({
            monitors: multiMonitors,
            isDark: true,
            volume: 75,
          });
        case 'get_monitors':
          return Promise.resolve(multiMonitors);
        case 'get_dark_mode':
          return Promise.resolve(true);
        case 'get_volume':
          return Promise.resolve(75);
        case 'get_app_version':
          return Promise.resolve('2.1.0');
        default:
          return Promise.resolve(undefined);
      }
    });

    const user = userEvent.setup();
    render(<App />);

    // Collapsed view with 3 monitors
    await waitFor(() => {
      expect(screen.getByText('All Monitors (3)')).toBeInTheDocument();
    });

    // Expand to individual monitors
    await user.click(screen.getByTitle('Show individual monitors'));
    await waitFor(() => {
      expect(screen.getByText('Built-in Display')).toBeInTheDocument();
      expect(screen.getByText('Dell U2723QE')).toBeInTheDocument();
      // Monitor with empty name should fall back to originalName
      expect(screen.getByText('LG 27UK850')).toBeInTheDocument();
    });

    expect(errorSpy).not.toHaveBeenCalled();
    errorSpy.mockRestore();
  });

  it('calls rename_monitor with uid (not id)', async () => {
    const user = userEvent.setup();
    render(<App />);

    // Expand to individual monitors
    await waitFor(() => {
      expect(screen.getByText('All Monitors (1)')).toBeInTheDocument();
    });
    await user.click(screen.getByTitle('Show individual monitors'));
    await waitFor(() => {
      expect(screen.getByText('Built-in Display')).toBeInTheDocument();
    });

    // Click monitor name to edit
    await user.click(screen.getByText('Built-in Display'));
    const input = screen.getByRole('textbox');
    await user.clear(input);
    await user.type(input, 'MacBook{Enter}');

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('rename_monitor', {
        uid: 'builtin-0::Built-in Display',
        name: 'MacBook',
      });
    });
  });

  it('calls set_dark_mode when DARK button clicked', async () => {
    const user = userEvent.setup();
    render(<App />);
    await waitFor(() => expect(screen.getByText('Display DJ')).toBeInTheDocument());
    await user.click(screen.getByText('DARK').closest('button')!);
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('set_dark_mode', { enabled: true });
    });
  });

  it('calls set_dark_mode false when LIGHT button clicked', async () => {
    const user = userEvent.setup();
    render(<App />);
    await waitFor(() => expect(screen.getByText('Display DJ')).toBeInTheDocument());
    await user.click(screen.getByText('LIGHT').closest('button')!);
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('set_dark_mode', { enabled: false });
    });
  });

  it('calls set_keep_awake when Keep Awake button is clicked', async () => {
    const user = userEvent.setup();
    render(<App />);
    await waitFor(() => expect(screen.getByText('Keep Awake: Off')).toBeInTheDocument());
    await user.click(screen.getByText('Keep Awake: Off'));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('set_keep_awake', { enabled: true });
    });
  });

  it('calls apply_profile when a profile button is clicked', async () => {
    const user = userEvent.setup();
    render(<App />);
    await waitFor(() => expect(screen.getByText('Presentation')).toBeInTheDocument());
    await user.click(screen.getByText('Focus'));
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('apply_profile', { index: 1 });
    });
  });

  it('opens settings panel when settings button is clicked', async () => {
    const user = userEvent.setup();
    render(<App />);
    await waitFor(() => expect(screen.getByText('Display DJ')).toBeInTheDocument());
    await user.click(screen.getByTitle('Settings'));
    await waitFor(() => expect(screen.getByText('Settings')).toBeInTheDocument());
  });

  it('handles backend rejection on dark-mode toggle without crashing', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'set_dark_mode') return Promise.reject(new Error('nope'));
      if (cmd === 'fetch_all_state') {
        return Promise.resolve({ monitors: [], isDark: false, volume: 50 });
      }
      if (cmd === 'get_app_version') return Promise.resolve('2.0.0');
      if (cmd === 'get_preferences') {
        return Promise.resolve({
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
          },
          debugLogging: false,
          launchAtLogin: false,
          monitorConfigs: [],
        });
      }
      return Promise.resolve(undefined);
    });
    const errSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    const user = userEvent.setup();
    render(<App />);
    await waitFor(() => expect(screen.getByText('DARK', { exact: false })).toBeInTheDocument());
    await user.click(screen.getByText('DARK').closest('button')!);
    await waitFor(() => {
      expect(errSpy).toHaveBeenCalledWith('Failed to set dark mode:', expect.any(Error));
    });
    errSpy.mockRestore();
  });

  it('calls set_brightness with API id (not uid)', async () => {
    const singleMonitor = [
      {
        id: '1',
        uid: '1::Dell U2723QE',
        name: 'Dell U2723QE',
        originalName: 'Dell U2723QE',
        brightness: 50,
        supportsBrightness: true,
        isBuiltIn: false,
      },
    ];
    mockInvoke.mockImplementation((cmd: string) => {
      switch (cmd) {
        case 'fetch_all_state':
          return Promise.resolve({
            monitors: singleMonitor,
            isDark: false,
            volume: 50,
          });
        case 'get_monitors':
          return Promise.resolve(singleMonitor);
        case 'get_dark_mode':
          return Promise.resolve(false);
        case 'get_volume':
          return Promise.resolve(50);
        case 'get_preferences':
          return Promise.resolve({
            showIndividualDisplays: false,
            minBrightness: 5,
            keyBindings: [],
            profiles: [],
            nightModeSchedule: {
              enabled: false,
              nightStart: '21:00',
              nightBrightness: 20,
              dayStart: '07:00',
              dayBrightness: 100,
            },
            debugLogging: false,
            launchAtLogin: false,
            monitorConfigs: [],
          });
        case 'get_app_version':
          return Promise.resolve('2.1.0');
        case 'set_brightness':
          return Promise.resolve(undefined);
        default:
          return Promise.resolve(undefined);
      }
    });

    const user = userEvent.setup();
    render(<App />);

    // Expand to individual monitors
    await waitFor(() => {
      expect(screen.getByText('All Monitors (1)')).toBeInTheDocument();
    });
    await user.click(screen.getByTitle('Show individual monitors'));
    await waitFor(() => {
      expect(screen.getByText('Dell U2723QE')).toBeInTheDocument();
    });

    // Click the monitor icon to toggle brightness (triggers set_brightness)
    const icon = screen.getByText('\uD83D\uDDA5');
    await user.click(icon);

    await waitFor(() => {
      // Should use API id "1" (not uid "1::Dell U2723QE")
      expect(mockInvoke).toHaveBeenCalledWith('set_brightness', {
        monitorId: '1',
        value: 5,
      });
    });
  });

  it('calls set_contrast with monitorId when contrast icon is toggled on a single monitor', async () => {
    const singleWithContrast = [
      {
        id: '1',
        uid: '1::Dell U2723QE',
        name: 'Dell U2723QE',
        originalName: 'Dell U2723QE',
        brightness: 50,
        contrast: 70,
        supportsBrightness: true,
        isBuiltIn: false,
      },
    ];
    mockInvoke.mockImplementation((cmd: string) => {
      switch (cmd) {
        case 'fetch_all_state':
          return Promise.resolve({ monitors: singleWithContrast, isDark: false, volume: 50 });
        case 'get_monitors':
          return Promise.resolve(singleWithContrast);
        case 'get_preferences':
          return Promise.resolve({
            showIndividualDisplays: true,
            showContrast: true,
            minBrightness: 5,
            keyBindings: [],
            profiles: [],
            nightModeSchedule: {
              enabled: false,
              nightStart: '21:00',
              nightBrightness: 20,
              dayStart: '07:00',
              dayBrightness: 100,
            },
            debugLogging: false,
            launchAtLogin: false,
            monitorConfigs: [],
          });
        case 'get_app_version':
          return Promise.resolve('2.1.0');
        default:
          return Promise.resolve(undefined);
      }
    });

    const user = userEvent.setup();
    render(<App />);

    await waitFor(() => {
      expect(screen.getByText('All Monitors (1)')).toBeInTheDocument();
    });
    // Expand to individual monitors to expose the per-monitor contrast slider
    await user.click(screen.getByTitle('Show individual monitors'));
    await waitFor(() => {
      expect(screen.getByText('Dell U2723QE')).toBeInTheDocument();
    });

    // The contrast icon (◐) is rendered next to each monitor's contrast slider.
    const contrastIcons = screen.getAllByText('\u25D0');
    // Click the per-monitor contrast icon; current value 70 > 0 so handler sends 0.
    await user.click(contrastIcons[contrastIcons.length - 1]);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith('set_contrast', {
        monitorId: '1',
        value: 0,
      });
    });
  });

  it('calls save_monitor_order when reorder buttons are used', async () => {
    const multi = [
      {
        id: '1',
        uid: '1::Dell U2723QE',
        name: 'Dell U2723QE',
        originalName: 'Dell U2723QE',
        brightness: 80,
        supportsBrightness: true,
        isBuiltIn: false,
      },
      {
        id: '2',
        uid: '2::LG 27UK850',
        name: 'LG 27UK850',
        originalName: 'LG 27UK850',
        brightness: 60,
        supportsBrightness: true,
        isBuiltIn: false,
      },
    ];
    mockInvoke.mockImplementation((cmd: string) => {
      switch (cmd) {
        case 'fetch_all_state':
          return Promise.resolve({ monitors: multi, isDark: false, volume: 50 });
        case 'get_monitors':
          return Promise.resolve(multi);
        case 'get_preferences':
          return Promise.resolve({
            showIndividualDisplays: true,
            minBrightness: 5,
            keyBindings: [],
            profiles: [],
            nightModeSchedule: {
              enabled: false,
              nightStart: '21:00',
              nightBrightness: 20,
              dayStart: '07:00',
              dayBrightness: 100,
            },
            debugLogging: false,
            launchAtLogin: false,
            monitorConfigs: [],
          });
        case 'get_app_version':
          return Promise.resolve('2.1.0');
        case 'save_monitor_order':
          return Promise.resolve(undefined);
        default:
          return Promise.resolve(undefined);
      }
    });

    const user = userEvent.setup();
    render(<App />);

    await waitFor(() => expect(screen.getByText('All Monitors (2)')).toBeInTheDocument());
    await user.click(screen.getByTitle('Show individual monitors'));
    await waitFor(() => expect(screen.getByText('Dell U2723QE')).toBeInTheDocument());

    // Move the first monitor down (only Move down on index 0 is enabled).
    const moveDownButtons = screen.getAllByTitle('Move down');
    await user.click(moveDownButtons[0]);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        'save_monitor_order',
        expect.objectContaining({
          orders: expect.arrayContaining([
            ['1::Dell U2723QE', 1],
            ['2::LG 27UK850', 0],
          ]),
        }),
      );
    });
  });

  it('does nothing when reorder hits a boundary (move up on first monitor)', async () => {
    // Same as the previous reorder test, but click "Move up" on index 0 which is disabled —
    // save_monitor_order must not fire.
    const multi = [
      {
        id: '1',
        uid: '1::Dell U2723QE',
        name: 'Dell U2723QE',
        originalName: 'Dell U2723QE',
        brightness: 80,
        supportsBrightness: true,
        isBuiltIn: false,
      },
      {
        id: '2',
        uid: '2::LG 27UK850',
        name: 'LG 27UK850',
        originalName: 'LG 27UK850',
        brightness: 60,
        supportsBrightness: true,
        isBuiltIn: false,
      },
    ];
    mockInvoke.mockImplementation((cmd: string) => {
      switch (cmd) {
        case 'fetch_all_state':
          return Promise.resolve({ monitors: multi, isDark: false, volume: 50 });
        case 'get_monitors':
          return Promise.resolve(multi);
        case 'get_preferences':
          return Promise.resolve({
            showIndividualDisplays: true,
            minBrightness: 5,
            keyBindings: [],
            profiles: [],
            nightModeSchedule: {
              enabled: false,
              nightStart: '21:00',
              nightBrightness: 20,
              dayStart: '07:00',
              dayBrightness: 100,
            },
            debugLogging: false,
            launchAtLogin: false,
            monitorConfigs: [],
          });
        case 'get_app_version':
          return Promise.resolve('2.1.0');
        default:
          return Promise.resolve(undefined);
      }
    });

    const user = userEvent.setup();
    render(<App />);
    await waitFor(() => expect(screen.getByText('All Monitors (2)')).toBeInTheDocument());
    await user.click(screen.getByTitle('Show individual monitors'));
    await waitFor(() => expect(screen.getByText('Dell U2723QE')).toBeInTheDocument());

    const moveUpButtons = screen.getAllByTitle('Move up');
    expect(moveUpButtons[0]).toBeDisabled();

    // Even if we try to click, the button is disabled so handler never fires.
    await user.click(moveUpButtons[0]);
    expect(mockInvoke).not.toHaveBeenCalledWith('save_monitor_order', expect.anything());
  });

  it('opens the About panel when the backend emits show-about', async () => {
    // Capture the handler registered for the "show-about" event so the test
    // can fire it synchronously, mirroring the runtime backend emit.
    let showAboutHandler: ((event: { payload: unknown }) => void) | null = null;
    mockListen.mockImplementation(
      (event: string, handler: (event: { payload: unknown }) => void) => {
        if (event === 'show-about') {
          showAboutHandler = handler;
        }
        return Promise.resolve(() => {});
      },
    );

    // Extend the default invoke mock to also answer get_about_info so the
    // AboutPanel can mount without crashing on `info.os` reads.
    const baseImpl = mockInvoke.getMockImplementation()!;
    mockInvoke.mockImplementation((cmd: string, args?: unknown) => {
      if (cmd === 'get_about_info') {
        return Promise.resolve({
          version: '2.1.0',
          os: 'macOS',
          arch: 'arm64',
          engine: 'Tauri',
          homepage: 'https://github.com/synle/display-dj',
          buildDate: '2026-05-18',
        });
      }
      return baseImpl(cmd, args);
    });

    // AboutPanel issues a real fetch to GitHub — stub it so the test doesn't
    // hit the network and doesn't trigger an unhandled rejection in jsdom.
    const fetchSpy = vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      new Response(JSON.stringify({ tag_name: 'v2.1.0', published_at: '2026-05-13T22:46:56Z' }), {
        status: 200,
      }),
    );

    render(<App />);

    // Wait for App to render so the listen() registration has happened.
    await waitFor(() => expect(screen.getByText('Display DJ')).toBeInTheDocument());

    // Fire the show-about event the way the backend would.
    expect(showAboutHandler).not.toBeNull();
    act(() => {
      showAboutHandler!({ payload: null });
    });

    // The panel renders an "About" title.
    await waitFor(() => {
      expect(screen.getByText('About')).toBeInTheDocument();
    });

    fetchSpy.mockRestore();
  });

  it('logs an error when set_volume rejects', async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === 'fetch_all_state') {
        return Promise.resolve({ monitors: [], isDark: false, volume: 50 });
      }
      if (cmd === 'get_app_version') return Promise.resolve('2.1.0');
      if (cmd === 'set_volume') return Promise.reject(new Error('volume unavailable'));
      if (cmd === 'get_preferences') {
        return Promise.resolve({
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
          },
          debugLogging: false,
          launchAtLogin: false,
          monitorConfigs: [],
        });
      }
      return Promise.resolve(undefined);
    });

    const errSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    const user = userEvent.setup();
    render(<App />);

    await waitFor(() => expect(screen.getByText('Display DJ')).toBeInTheDocument());

    // The volume slider's mute icon click triggers handleVolume(0).
    // There are multiple volume icons rendered depending on state; click the volume Slider.
    const volumeIcon = screen.getByText('\uD83D\uDD0A'); // 🔊
    await user.click(volumeIcon);

    await waitFor(() => {
      expect(errSpy).toHaveBeenCalledWith('Failed to set volume:', expect.any(Error));
    });
    errSpy.mockRestore();
  });
});
