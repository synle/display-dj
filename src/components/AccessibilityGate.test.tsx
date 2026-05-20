import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import AccessibilityGate from './AccessibilityGate';

const mockedInvoke = invoke as unknown as ReturnType<typeof vi.fn>;

describe('AccessibilityGate', () => {
  beforeEach(() => {
    mockedInvoke.mockReset();
  });

  it('renders the permission title and step list', () => {
    render(<AccessibilityGate onGranted={() => {}} />);
    expect(screen.getByText('Accessibility permission required')).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: /Open Accessibility Settings/i }),
    ).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /recheck/i })).toBeInTheDocument();
  });

  it('invokes open_accessibility_settings when the open button is clicked', async () => {
    mockedInvoke.mockResolvedValue(undefined);
    const user = userEvent.setup();
    render(<AccessibilityGate onGranted={() => {}} />);
    await user.click(screen.getByRole('button', { name: /Open Accessibility Settings/i }));
    expect(mockedInvoke).toHaveBeenCalledWith('open_accessibility_settings');
  });

  it('calls onGranted when recheck returns true', async () => {
    mockedInvoke.mockResolvedValueOnce(true);
    const onGranted = vi.fn();
    const user = userEvent.setup();
    render(<AccessibilityGate onGranted={onGranted} />);
    await user.click(screen.getByRole('button', { name: /recheck/i }));
    await waitFor(() => expect(onGranted).toHaveBeenCalledTimes(1));
    expect(mockedInvoke).toHaveBeenCalledWith('recheck_accessibility_trusted');
  });

  it('shows a hint when recheck still returns false', async () => {
    mockedInvoke.mockResolvedValueOnce(false);
    const onGranted = vi.fn();
    const user = userEvent.setup();
    render(<AccessibilityGate onGranted={onGranted} />);
    await user.click(screen.getByRole('button', { name: /recheck/i }));
    await waitFor(() => expect(screen.getByText(/Still not granted/i)).toBeInTheDocument());
    expect(onGranted).not.toHaveBeenCalled();
  });

  it('surfaces a hint when recheck rejects', async () => {
    mockedInvoke.mockRejectedValueOnce(new Error('boom'));
    const onGranted = vi.fn();
    const user = userEvent.setup();
    render(<AccessibilityGate onGranted={onGranted} />);
    await user.click(screen.getByRole('button', { name: /recheck/i }));
    await waitFor(() => expect(screen.getByText(/Still not granted/i)).toBeInTheDocument());
    expect(onGranted).not.toHaveBeenCalled();
  });
});
