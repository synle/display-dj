import { render, screen, act, fireEvent } from '@testing-library/react';
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
    render(<Slider label='Brightness' icon='☀' value={50} onChange={() => {}} />);
    expect(screen.getByText('☀')).toBeInTheDocument();
  });

  it('displays the current value as percentage', () => {
    render(<Slider label='Brightness' icon='☀' value={75} onChange={() => {}} />);
    expect(screen.getByText('75%')).toBeInTheDocument();
  });

  it('hides value when showValue is false', () => {
    render(<Slider label='Brightness' icon='☀' value={75} onChange={() => {}} showValue={false} />);
    expect(screen.queryByText('75%')).not.toBeInTheDocument();
  });

  it('renders a range input with correct min/max/value', () => {
    render(<Slider label='Brightness' icon='☀' value={60} min={0} max={100} onChange={() => {}} />);
    const input = screen.getByRole('slider', { name: 'Brightness' });
    expect(input).toHaveAttribute('min', '0');
    expect(input).toHaveAttribute('max', '100');
    expect(input).toHaveValue('60');
  });

  it('debounces onChange calls', async () => {
    const onChange = vi.fn();
    render(<Slider label='Brightness' icon='☀' value={50} onChange={onChange} />);
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
      vi.advanceTimersByTime(50);
    });

    expect(onChange).toHaveBeenCalledWith(70);
  });

  it('sets correct fill width based on value', () => {
    const { container } = render(
      <Slider label='Brightness' icon='☀' value={50} min={0} max={100} onChange={() => {}} />,
    );
    const fill = container.querySelector('.slider-fill') as HTMLElement;
    expect(fill.style.width).toBe('50%');
  });

  it('calculates fill correctly with custom min/max', () => {
    const { container } = render(
      <Slider label='Brightness' icon='☀' value={75} min={50} max={100} onChange={() => {}} />,
    );
    const fill = container.querySelector('.slider-fill') as HTMLElement;
    expect(fill.style.width).toBe('50%');
  });

  it('updates local value when prop changes', () => {
    const { rerender } = render(
      <Slider label='Brightness' icon='☀' value={50} onChange={() => {}} />,
    );
    expect(screen.getByText('50%')).toBeInTheDocument();

    rerender(<Slider label='Brightness' icon='☀' value={80} onChange={() => {}} />);
    expect(screen.getByText('80%')).toBeInTheDocument();
  });

  /** Clickable slider icons expose native button semantics and a useful name. */
  it('renders a clickable icon as a labeled button', () => {
    render(
      <Slider
        label='Brightness'
        icon='☀'
        iconLabel='Toggle brightness'
        value={50}
        onChange={() => {}}
        onIconClick={() => {}}
      />,
    );

    expect(screen.getByRole('button', { name: 'Toggle brightness' })).toBeInTheDocument();
  });
  /** throttleMs sends the first drag value immediately and the final one after the window. */
  it('sends leading and trailing values when throttled', () => {
    const onChange = vi.fn();
    render(<Slider label='Volume' value={50} onChange={onChange} throttleMs={30} />);
    const input = screen.getByRole('slider', { name: 'Volume' });

    fireEvent.change(input, { target: { value: '60' } });
    expect(onChange).toHaveBeenCalledWith(60);
    fireEvent.change(input, { target: { value: '65' } });
    fireEvent.change(input, { target: { value: '70' } });
    expect(onChange).toHaveBeenCalledTimes(1);

    act(() => vi.advanceTimersByTime(30));
    expect(onChange).toHaveBeenCalledTimes(2);
    expect(onChange).toHaveBeenLastCalledWith(70);
  });

  /** Without throttleMs, only the final value is sent after a 50ms quiet period. */
  it('debounces to the final value when not throttled', () => {
    const onChange = vi.fn();
    render(<Slider label='Brightness' value={50} onChange={onChange} />);
    const input = screen.getByRole('slider', { name: 'Brightness' });

    fireEvent.change(input, { target: { value: '60' } });
    fireEvent.change(input, { target: { value: '70' } });
    act(() => vi.advanceTimersByTime(49));
    expect(onChange).not.toHaveBeenCalled();
    act(() => vi.advanceTimersByTime(1));
    expect(onChange).toHaveBeenCalledTimes(1);
    expect(onChange).toHaveBeenCalledWith(70);
  });
});
