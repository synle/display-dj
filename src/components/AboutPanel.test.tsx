import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import AboutPanel from './AboutPanel';

const mockInvoke = vi.mocked(invoke);

/** Build a stub Response-like object for global.fetch mocks. */
function jsonResponse(body: unknown) {
  return Promise.resolve({ json: () => Promise.resolve(body) }) as Promise<Response>;
}

/** Configure invoke + fetch mocks for the AboutPanel scenarios. */
function setupMocks(opts: {
  info?: Record<string, string>;
  currentVersion?: string;
  latestTag?: string;
  fetchRejects?: boolean;
  infoRejects?: boolean;
}) {
  mockInvoke.mockReset();
  mockInvoke.mockImplementation((cmd: string) => {
    if (cmd === 'get_about_info') {
      if (opts.infoRejects) return Promise.reject(new Error('boom'));
      return Promise.resolve(opts.info ?? {});
    }
    if (cmd === 'get_app_version') {
      return Promise.resolve(opts.currentVersion ?? '2.0.0');
    }
    return Promise.resolve(undefined);
  });

  global.fetch = vi.fn(() => {
    if (opts.fetchRejects) return Promise.reject(new Error('offline'));
    return jsonResponse({ tag_name: opts.latestTag ?? 'v2.0.0' });
  }) as unknown as typeof fetch;
}

describe('AboutPanel', () => {
  beforeEach(() => {
    vi.spyOn(console, 'error').mockImplementation(() => {});
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('renders panel chrome and calls onClose when the close button is clicked', async () => {
    setupMocks({ info: { version: '2.0.0', os: 'Linux', arch: 'x64' } });
    const onClose = vi.fn();
    const user = userEvent.setup();
    render(<AboutPanel onClose={onClose} />);

    expect(screen.getByText('About')).toBeInTheDocument();
    await user.click(screen.getByTitle('Close'));
    expect(onClose).toHaveBeenCalledOnce();
  });

  it('shows "Up to date" badge when current >= latest', async () => {
    setupMocks({
      info: { version: '2.0.0', os: 'Linux', arch: 'x64' },
      currentVersion: '2.0.0',
      latestTag: 'v2.0.0',
    });
    render(<AboutPanel onClose={() => {}} />);

    await waitFor(() => {
      expect(screen.getByText('Up to date')).toBeInTheDocument();
    });
  });

  it('shows "Update available" badge and download link when latest > current', async () => {
    setupMocks({
      info: { version: '1.0.0', os: 'Linux', arch: 'x64' },
      currentVersion: '1.0.0',
      latestTag: 'v2.0.0',
    });
    render(<AboutPanel onClose={() => {}} />);

    await waitFor(() => {
      expect(screen.getByText('Update available')).toBeInTheDocument();
      expect(screen.getByText('Download v2.0.0')).toBeInTheDocument();
    });
  });

  it('renders macOS troubleshooting section when os == "macOS"', async () => {
    setupMocks({
      info: { version: '2.0.0', os: 'macOS', arch: 'arm64' },
      currentVersion: '2.0.0',
      latestTag: 'v2.0.0',
    });
    render(<AboutPanel onClose={() => {}} />);

    await waitFor(() => {
      expect(screen.getByText(/macOS Troubleshooting/)).toBeInTheDocument();
      expect(screen.getByText(/xattr -cr/)).toBeInTheDocument();
    });
  });

  it('does not render macOS troubleshooting on non-macOS', async () => {
    setupMocks({
      info: { version: '2.0.0', os: 'Linux', arch: 'x64' },
      currentVersion: '2.0.0',
      latestTag: 'v2.0.0',
    });
    render(<AboutPanel onClose={() => {}} />);

    await waitFor(() => {
      expect(screen.getByText('Up to date')).toBeInTheDocument();
    });
    expect(screen.queryByText(/macOS Troubleshooting/)).not.toBeInTheDocument();
  });

  it('handles fetch failure by falling back to "unknown"/up-to-date', async () => {
    setupMocks({
      info: { version: '2.0.0', os: 'Linux', arch: 'x64' },
      currentVersion: '2.0.0',
      fetchRejects: true,
    });
    render(<AboutPanel onClose={() => {}} />);

    await waitFor(() => {
      expect(screen.getByText('Up to date')).toBeInTheDocument();
      expect(screen.getByText('unknown')).toBeInTheDocument();
    });
  });

  it('handles missing tag_name from the API as "unknown"', async () => {
    setupMocks({
      info: { version: '2.0.0', os: 'Linux', arch: 'x64' },
      currentVersion: '2.0.0',
      latestTag: undefined,
    });
    // Make the fetch payload empty (no tag_name) to trigger the "unknown" branch.
    global.fetch = vi.fn(() =>
      Promise.resolve({ json: () => Promise.resolve({}) }),
    ) as unknown as typeof fetch;
    render(<AboutPanel onClose={() => {}} />);

    await waitFor(() => {
      expect(screen.getByText('Up to date')).toBeInTheDocument();
      expect(screen.getByText('unknown')).toBeInTheDocument();
    });
  });

  it('handles get_about_info rejection without crashing', async () => {
    setupMocks({
      infoRejects: true,
      currentVersion: '2.0.0',
      latestTag: 'v2.0.0',
    });
    const { container } = render(<AboutPanel onClose={() => {}} />);
    await waitFor(() => {
      expect(container.querySelector('.settings-panel')).toBeInTheDocument();
    });
  });

  it('renders platform and homepage rows from info', async () => {
    setupMocks({
      info: {
        version: '2.0.0',
        os: 'Linux',
        arch: 'x86_64',
        engine: 'Tauri 2.0',
        buildDate: '2025-01-01',
        homepage: 'https://example.com',
      },
      currentVersion: '2.0.0',
      latestTag: 'v2.0.0',
    });
    render(<AboutPanel onClose={() => {}} />);

    await waitFor(() => {
      expect(screen.getByText('Tauri 2.0')).toBeInTheDocument();
      expect(screen.getByText('2025-01-01')).toBeInTheDocument();
      expect(screen.getByText(/Linux/)).toBeInTheDocument();
    });
    expect(screen.getByText('synle/display-dj').getAttribute('href')).toBe('https://example.com');
  });
});
