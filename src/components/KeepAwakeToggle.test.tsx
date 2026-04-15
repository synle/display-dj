import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, it, expect, vi } from 'vitest';
import KeepAwakeToggle from './KeepAwakeToggle';

describe('KeepAwakeToggle', () => {
  it('renders with inactive state', () => {
    render(<KeepAwakeToggle isActive={false} onChange={() => {}} />);
    expect(screen.getByText('Keep Awake: Off')).toBeInTheDocument();
  });

  it('renders with active state', () => {
    render(<KeepAwakeToggle isActive={true} onChange={() => {}} />);
    expect(screen.getByText('Keep Awake')).toBeInTheDocument();
  });

  it('applies active class when active', () => {
    render(<KeepAwakeToggle isActive={true} onChange={() => {}} />);
    const btn = screen.getByRole('button');
    expect(btn).toHaveClass('active');
  });

  it('does not apply active class when inactive', () => {
    render(<KeepAwakeToggle isActive={false} onChange={() => {}} />);
    const btn = screen.getByRole('button');
    expect(btn).not.toHaveClass('active');
  });

  it('calls onChange with true when clicked while inactive', async () => {
    const onChange = vi.fn();
    const user = userEvent.setup();
    render(<KeepAwakeToggle isActive={false} onChange={onChange} />);

    await user.click(screen.getByRole('button'));
    expect(onChange).toHaveBeenCalledWith(true);
  });

  it('calls onChange with false when clicked while active', async () => {
    const onChange = vi.fn();
    const user = userEvent.setup();
    render(<KeepAwakeToggle isActive={true} onChange={onChange} />);

    await user.click(screen.getByRole('button'));
    expect(onChange).toHaveBeenCalledWith(false);
  });

  it('shows correct tooltip when inactive', () => {
    render(<KeepAwakeToggle isActive={false} onChange={() => {}} />);
    const btn = screen.getByRole('button');
    expect(btn).toHaveAttribute('title', 'Click to prevent the system from sleeping');
  });

  it('shows correct tooltip when active', () => {
    render(<KeepAwakeToggle isActive={true} onChange={() => {}} />);
    const btn = screen.getByRole('button');
    expect(btn).toHaveAttribute('title', 'System is being kept awake — click to allow sleep');
  });

  it('renders coffee icon', () => {
    render(<KeepAwakeToggle isActive={false} onChange={() => {}} />);
    expect(screen.getByText('\u2615')).toBeInTheDocument();
  });
});
