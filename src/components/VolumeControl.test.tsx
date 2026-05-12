import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, it, expect, vi } from 'vitest';
import VolumeControl from './VolumeControl';

describe('VolumeControl', () => {
  it('renders a slider with the given volume value', () => {
    render(<VolumeControl value={75} onChange={() => {}} />);
    const slider = screen.getByRole('slider');
    expect(slider).toHaveValue('75');
  });

  it('shows muted icon when volume is 0', () => {
    render(<VolumeControl value={0} onChange={() => {}} />);
    expect(screen.getByText('\uD83D\uDD07')).toBeInTheDocument();
  });

  it('shows speaker icon when volume is above 0', () => {
    render(<VolumeControl value={50} onChange={() => {}} />);
    expect(screen.getByText('\uD83D\uDD0A')).toBeInTheDocument();
  });

  it('mutes (calls onChange with 0) when icon clicked at non-zero volume', async () => {
    const onChange = vi.fn();
    const user = userEvent.setup();
    render(<VolumeControl value={50} onChange={onChange} />);
    await user.click(screen.getByText('\uD83D\uDD0A'));
    expect(onChange).toHaveBeenCalledWith(0);
  });

  it('unmutes (calls onChange with 100) when icon clicked at zero volume', async () => {
    const onChange = vi.fn();
    const user = userEvent.setup();
    render(<VolumeControl value={0} onChange={onChange} />);
    await user.click(screen.getByText('\uD83D\uDD07'));
    expect(onChange).toHaveBeenCalledWith(100);
  });
});
