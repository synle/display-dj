import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, it, expect, vi } from 'vitest';
import KeyboardBacklightControl from './KeyboardBacklightControl';

describe('KeyboardBacklightControl', () => {
  it('renders a slider with the given backlight value', () => {
    render(<KeyboardBacklightControl value={50} onChange={() => {}} />);
    const slider = screen.getByRole('slider');
    expect(slider).toHaveValue('50');
  });

  it('sets step=25 on the underlying input so only 0/25/50/75/100 are reachable', () => {
    render(<KeyboardBacklightControl value={50} onChange={() => {}} />);
    const slider = screen.getByRole('slider');
    expect(slider).toHaveAttribute('step', '25');
  });

  it('renders the keyboard symbol (U+2328) as the icon', () => {
    render(<KeyboardBacklightControl value={50} onChange={() => {}} />);
    expect(screen.getByText('\u2328')).toBeInTheDocument();
  });

  it('clears (calls onChange with 0) when icon is clicked at a non-zero level', async () => {
    const onChange = vi.fn();
    const user = userEvent.setup();
    render(<KeyboardBacklightControl value={50} onChange={onChange} />);
    await user.click(screen.getByText('\u2328'));
    expect(onChange).toHaveBeenCalledWith(0);
  });

  it('restores (calls onChange with 100) when icon is clicked at level zero', async () => {
    const onChange = vi.fn();
    const user = userEvent.setup();
    render(<KeyboardBacklightControl value={0} onChange={onChange} />);
    await user.click(screen.getByText('\u2328'));
    expect(onChange).toHaveBeenCalledWith(100);
  });
});
