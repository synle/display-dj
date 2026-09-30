import { useCallback, useLayoutEffect, useRef, useState, type ReactNode } from 'react';

/**
 * Props for {@link Tooltip}.
 * @property text - Short description shown on hover/focus.
 * @property children - The control (checkbox row, label, …) the tooltip describes.
 */
interface TooltipProps {
  text: ReactNode;
  children: ReactNode;
}

/** Gap between the control and the bubble, in CSS pixels. */
const GAP_PX = 4;
/** Minimum distance kept between the bubble and the viewport edges. */
const EDGE_MARGIN_PX = 6;

/**
 * Hover/focus tooltip wrapper. Renders children plus a `role="tooltip"` bubble.
 *
 * The bubble is `position: fixed` so it never enlarges the scrollable popup or
 * gets clipped by it. On show, it measures itself: it opens below the control,
 * flips above when the space below is too small, and is clamped horizontally
 * inside the viewport.
 */
export default function Tooltip({ text, children }: TooltipProps) {
  const wrapRef = useRef<HTMLDivElement>(null);
  const bubbleRef = useRef<HTMLSpanElement>(null);
  const [open, setOpen] = useState(false);
  const [pos, setPos] = useState<{ top: number; left: number } | null>(null);

  /** Compute the bubble's fixed position from the wrapper and bubble rects. */
  const place = useCallback(() => {
    const wrap = wrapRef.current;
    const bubble = bubbleRef.current;
    if (!wrap || !bubble) return;
    const anchor = wrap.getBoundingClientRect();
    const { width, height } = bubble.getBoundingClientRect();
    const vw = window.innerWidth;
    const vh = window.innerHeight;
    const fitsBelow = anchor.bottom + GAP_PX + height <= vh - EDGE_MARGIN_PX;
    const top = fitsBelow
      ? anchor.bottom + GAP_PX
      : Math.max(EDGE_MARGIN_PX, anchor.top - GAP_PX - height);
    const left = Math.min(
      Math.max(EDGE_MARGIN_PX, anchor.left),
      Math.max(EDGE_MARGIN_PX, vw - EDGE_MARGIN_PX - width),
    );
    setPos({ top, left });
  }, []);

  useLayoutEffect(() => {
    if (open) place();
    else setPos(null);
  }, [open, place]);

  return (
    <div
      ref={wrapRef}
      className='tooltip'
      onMouseEnter={() => setOpen(true)}
      onMouseLeave={() => setOpen(false)}
      onFocus={() => setOpen(true)}
      onBlur={() => setOpen(false)}>
      {children}
      <span
        ref={bubbleRef}
        className={`tooltip-bubble${open && pos ? ' tooltip-bubble-visible' : ''}`}
        role='tooltip'
        style={pos ? { top: pos.top, left: pos.left } : undefined}>
        {text}
      </span>
    </div>
  );
}
