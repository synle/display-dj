import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import App from './App';

const mockInvoke = vi.mocked(invoke);

beforeEach(() => {
  mockInvoke.mockReset();
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

    // Expand to show individual monitors
    await user.click(screen.getByTitle('Show individual monitors'));
    await waitFor(() => {
      expect(screen.getByText('Built-in Display')).toBeInTheDocument();
    });

    // Collapse back
    await user.click(screen.getByTitle('Show all monitors control'));
    await waitFor(() => {
      expect(screen.getByText('All Monitors (1)')).toBeInTheDocument();
    });

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
});
