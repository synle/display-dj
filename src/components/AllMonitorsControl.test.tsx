import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, it, expect, vi } from 'vitest';
import AllMonitorsControl from './AllMonitorsControl';

const defaultProps = {
  brightness: 50,
  onBrightnessChange: () => {},
  contrast: null as number | null,
  onContrastChange: () => {},
  showContrast: false,
  monitorCount: 3,
  minBrightness: 10,
  onExpand: () => {},
};

describe('AllMonitorsControl', () => {
  it("renders 'All Monitors' label with count", () => {
    render(<AllMonitorsControl {...defaultProps} />);
    expect(screen.getByText('All Monitors (3)')).toBeInTheDocument();
  });

  it('renders brightness slider with correct value', () => {
    render(<AllMonitorsControl {...defaultProps} brightness={70} />);
    const sliders = screen.getAllByRole('slider');
    expect(sliders).toHaveLength(1);
    expect(sliders[0]).toHaveValue('70');
  });

  it('renders expand button', () => {
    render(<AllMonitorsControl {...defaultProps} />);
    expect(screen.getByTitle('Show individual monitors')).toBeInTheDocument();
  });

  it('calls onExpand when expand button is clicked', async () => {
    const onExpand = vi.fn();
    const user = userEvent.setup();
    render(<AllMonitorsControl {...defaultProps} onExpand={onExpand} />);

    await user.click(screen.getByTitle('Show individual monitors'));
    expect(onExpand).toHaveBeenCalledOnce();
  });

  it('renders contrast slider when showContrast is true and contrast is non-null', () => {
    render(<AllMonitorsControl {...defaultProps} showContrast={true} contrast={42} />);
    const sliders = screen.getAllByRole('slider');
    expect(sliders).toHaveLength(2);
    expect(sliders[1]).toHaveValue('42');
  });

  it('hides contrast slider when showContrast is true but contrast is null', () => {
    render(<AllMonitorsControl {...defaultProps} showContrast={true} contrast={null} />);
    expect(screen.getAllByRole('slider')).toHaveLength(1);
  });

  it('hides contrast slider when showContrast is false even if contrast is non-null', () => {
    render(<AllMonitorsControl {...defaultProps} showContrast={false} contrast={42} />);
    expect(screen.getAllByRole('slider')).toHaveLength(1);
  });

  it('toggles brightness to minBrightness on icon click when above min', async () => {
    const onBrightnessChange = vi.fn();
    const user = userEvent.setup();
    render(
      <AllMonitorsControl
        {...defaultProps}
        brightness={80}
        minBrightness={10}
        onBrightnessChange={onBrightnessChange}
      />,
    );
    await user.click(screen.getByText('☀'));
    expect(onBrightnessChange).toHaveBeenCalledWith(10);
  });

  it('toggles brightness to 100 on icon click when at minBrightness', async () => {
    const onBrightnessChange = vi.fn();
    const user = userEvent.setup();
    render(
      <AllMonitorsControl
        {...defaultProps}
        brightness={10}
        minBrightness={10}
        onBrightnessChange={onBrightnessChange}
      />,
    );
    await user.click(screen.getByText('☀'));
    expect(onBrightnessChange).toHaveBeenCalledWith(100);
  });

  it('toggles contrast to 0 on icon click when above 0', async () => {
    const onContrastChange = vi.fn();
    const user = userEvent.setup();
    render(
      <AllMonitorsControl
        {...defaultProps}
        showContrast={true}
        contrast={50}
        onContrastChange={onContrastChange}
      />,
    );
    await user.click(screen.getByText('\u25D0'));
    expect(onContrastChange).toHaveBeenCalledWith(0);
  });

  it('toggles contrast to 100 on icon click when at 0', async () => {
    const onContrastChange = vi.fn();
    const user = userEvent.setup();
    render(
      <AllMonitorsControl
        {...defaultProps}
        showContrast={true}
        contrast={0}
        onContrastChange={onContrastChange}
      />,
    );
    await user.click(screen.getByText('\u25D0'));
    expect(onContrastChange).toHaveBeenCalledWith(100);
  });
});
