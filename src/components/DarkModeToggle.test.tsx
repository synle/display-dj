import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, it, expect, vi } from 'vitest';
import DarkModeToggle from './DarkModeToggle';

describe('DarkModeToggle', () => {
  it('renders Dark and Light buttons', () => {
    render(<DarkModeToggle isDarkMode={false} onChange={() => {}} />);
    expect(screen.getByText('DARK', { exact: false })).toBeInTheDocument();
    expect(screen.getByText('LIGHT', { exact: false })).toBeInTheDocument();
  });

  it('marks Dark button as active when dark mode is on', () => {
    render(<DarkModeToggle isDarkMode={true} onChange={() => {}} />);
    const buttons = screen.getAllByRole('button');
    const darkBtn = buttons.find((b) => b.textContent?.includes('DARK'))!;
    const lightBtn = buttons.find((b) => b.textContent?.includes('LIGHT'))!;
    expect(darkBtn).toHaveClass('active');
    expect(lightBtn).not.toHaveClass('active');
  });

  it('marks Light button as active when dark mode is off', () => {
    render(<DarkModeToggle isDarkMode={false} onChange={() => {}} />);
    const buttons = screen.getAllByRole('button');
    const darkBtn = buttons.find((b) => b.textContent?.includes('DARK'))!;
    const lightBtn = buttons.find((b) => b.textContent?.includes('LIGHT'))!;
    expect(darkBtn).not.toHaveClass('active');
    expect(lightBtn).toHaveClass('active');
  });

  it('calls onChange(true) when Dark button is clicked', async () => {
    const onChange = vi.fn();
    const user = userEvent.setup();
    render(<DarkModeToggle isDarkMode={false} onChange={onChange} />);

    const buttons = screen.getAllByRole('button');
    const darkBtn = buttons.find((b) => b.textContent?.includes('DARK'))!;
    await user.click(darkBtn);
    expect(onChange).toHaveBeenCalledWith(true);
  });

  it('calls onChange(false) when Light button is clicked', async () => {
    const onChange = vi.fn();
    const user = userEvent.setup();
    render(<DarkModeToggle isDarkMode={true} onChange={onChange} />);

    const buttons = screen.getAllByRole('button');
    const lightBtn = buttons.find((b) => b.textContent?.includes('LIGHT'))!;
    await user.click(lightBtn);
    expect(onChange).toHaveBeenCalledWith(false);
  });
});
