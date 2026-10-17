import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, it, expect, vi } from 'vitest';
import Header from './Header';

const defaultProps = {
  version: '2.0.0',
  lockScreenAvailable: false,
  onLockScreen: () => {},
  onSettingsToggle: () => {},
  settingsOpen: false,
};

describe('Header', () => {
  it('renders the app title', () => {
    render(<Header {...defaultProps} />);
    expect(screen.getByText('Display DJ')).toBeInTheDocument();
  });

  it('displays the version when provided', () => {
    render(<Header {...defaultProps} />);
    expect(screen.getByText('v2.0.0')).toBeInTheDocument();
  });

  it('hides version when empty string', () => {
    render(<Header {...defaultProps} version='' />);
    expect(screen.queryByText(/^v/)).not.toBeInTheDocument();
  });

  it('renders the settings button', () => {
    render(<Header {...defaultProps} />);
    expect(screen.getByTitle('Settings')).toBeInTheDocument();
  });

  it('renders lock screen before settings when available', () => {
    render(<Header {...defaultProps} lockScreenAvailable={true} />);
    const buttons = screen.getAllByRole('button');

    expect(buttons[0]).toHaveAccessibleName('Lock screen');
    expect(buttons[1]).toHaveAccessibleName('Settings');
    expect(document.querySelector('[data-icon="logout"]')).not.toBeNull();
  });

  it('hides lock screen when unsupported', () => {
    render(<Header {...defaultProps} />);
    expect(screen.queryByRole('button', { name: 'Lock screen' })).not.toBeInTheDocument();
  });

  it('calls onLockScreen when lock screen is clicked', async () => {
    const onLockScreen = vi.fn();
    const user = userEvent.setup();
    render(<Header {...defaultProps} lockScreenAvailable={true} onLockScreen={onLockScreen} />);

    await user.click(screen.getByRole('button', { name: 'Lock screen' }));
    expect(onLockScreen).toHaveBeenCalledOnce();
  });

  it('calls onSettingsToggle when the settings button is clicked', async () => {
    const onSettingsToggle = vi.fn();
    const user = userEvent.setup();
    render(<Header {...defaultProps} onSettingsToggle={onSettingsToggle} />);

    await user.click(screen.getByTitle('Settings'));
    expect(onSettingsToggle).toHaveBeenCalledOnce();
  });

  it('applies active class to settings button when settings is open', () => {
    render(<Header {...defaultProps} settingsOpen={true} />);
    expect(screen.getByTitle('Settings')).toHaveClass('active');
  });

  it('does not apply active class to settings button when settings is closed', () => {
    render(<Header {...defaultProps} settingsOpen={false} />);
    expect(screen.getByTitle('Settings')).not.toHaveClass('active');
  });
});
