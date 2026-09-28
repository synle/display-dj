import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import RefreshLabel from './RefreshLabel';

describe('RefreshLabel', () => {
  /** Clicking the section label triggers a device rescan. */
  it('calls onRefresh when the label is clicked', () => {
    const onRefresh = vi.fn();
    render(<RefreshLabel text='All Monitors (2)' onRefresh={onRefresh} />);
    fireEvent.click(screen.getByRole('button', { name: /All Monitors \(2\)/ }));
    expect(onRefresh).toHaveBeenCalledTimes(1);
  });
});
