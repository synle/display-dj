import { render, screen, act } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import Slider from './Slider';

describe('Slider', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('renders the icon', () => {
    render(<Slider icon='☀' value={50} onChange={() => {}} />);
    expect(screen.getByText('☀')).toBeInTheDocument();
  });

  it('displays the current value as percentage', () => {
    render(<Slider icon='☀' value={75} onChange={() => {}} />);
    expect(screen.getByText('75%')).toBeInTheDocument();
  });

  it('hides value when showValue is false', () => {
    render(<Slider icon='☀' value={75} onChange={() => {}} showValue={false} />);
    expect(screen.queryByText('75%')).not.toBeInTheDocument();
  });

  it('renders a range input with correct min/max/value', () => {
    render(<Slider icon='☀' value={60} min={0} max={100} onChange={() => {}} />);
    const input = screen.getByRole('slider');
    expect(input).toHaveAttribute('min', '0');
    expect(input).toHaveAttribute('max', '100');
    expect(input).toHaveValue('60');
  });

  it('debounces onChange calls', async () => {
    const onChange = vi.fn();
    render(<Slider icon='☀' value={50} onChange={onChange} />);
    const input = screen.getByRole('slider');

    // Simulate changing the value
    await act(async () => {
      input.focus();
      // Fire native change event
      Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')!.set!.call(input, '70');
      input.dispatchEvent(new Event('change', { bubbles: true }));
    });

    // onChange should not be called immediately
    expect(onChange).not.toHaveBeenCalled();

    // After the debounce period
    await act(async () => {
      vi.advanceTimersByTime(150);
    });

    expect(onChange).toHaveBeenCalledWith(70);
  });

  it('sets correct fill width based on value', () => {
    const { container } = render(
      <Slider icon='☀' value={50} min={0} max={100} onChange={() => {}} />,
    );
    const fill = container.querySelector('.slider-fill') as HTMLElement;
    expect(fill.style.width).toBe('50%');
  });

  it('calculates fill correctly with custom min/max', () => {
    const { container } = render(
      <Slider icon='☀' value={75} min={50} max={100} onChange={() => {}} />,
    );
    const fill = container.querySelector('.slider-fill') as HTMLElement;
    expect(fill.style.width).toBe('50%');
  });

  it('updates local value when prop changes', () => {
    const { rerender } = render(<Slider icon='☀' value={50} onChange={() => {}} />);
    expect(screen.getByText('50%')).toBeInTheDocument();

    rerender(<Slider icon='☀' value={80} onChange={() => {}} />);
    expect(screen.getByText('80%')).toBeInTheDocument();
  });
});
