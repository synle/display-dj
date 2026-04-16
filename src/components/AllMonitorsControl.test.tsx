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
});
