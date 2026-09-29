import { describe, it, expect, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import Tooltip from './Tooltip';

/** Tooltip renders its child control plus a role=tooltip bubble with the description. */
describe('Tooltip', () => {
  /** Child stays rendered and the description is exposed as a tooltip. */
  it('renders children and the tooltip text', () => {
    render(
      <Tooltip text='Start on sign in.'>
        <label>
          <input type='checkbox' />
          <span>Launch at Login</span>
        </label>
      </Tooltip>,
    );
    expect(screen.getByLabelText('Launch at Login')).toBeInTheDocument();
    expect(screen.getByRole('tooltip')).toHaveTextContent('Start on sign in.');
  });

  /** Near the viewport bottom the bubble flips above the control instead of growing the page. */
  it('opens above the control when there is no room below', () => {
    const rect = (top: number, height: number, width = 100) =>
      ({
        top,
        bottom: top + height,
        left: 10,
        right: 10 + width,
        width,
        height,
        x: 10,
        y: top,
      }) as DOMRect;
    vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(
      function (this: HTMLElement) {
        return this.getAttribute('role') === 'tooltip'
          ? rect(0, 40)
          : rect(window.innerHeight - 20, 20);
      },
    );
    render(
      <Tooltip text='Start on sign in.'>
        <span>Launch at Login</span>
      </Tooltip>,
    );
    fireEvent.mouseEnter(screen.getByText('Launch at Login'));
    const bubble = screen.getByRole('tooltip');
    expect(bubble).toHaveClass('tooltip-bubble-visible');
    expect(bubble.style.top).toBe(`${window.innerHeight - 20 - 4 - 40}px`);
    vi.restoreAllMocks();
  });
});
