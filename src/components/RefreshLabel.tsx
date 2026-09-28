interface RefreshLabelProps {
  /** Visible section text, e.g. "All Monitors (2)". */
  text: string;
  /** Extra class names appended to `section-label`. */
  className?: string;
  /** Called when the label is clicked. */
  onRefresh: () => void;
}

/** Section label that rescans displays/speakers when clicked, with a green refresh icon. */
export default function RefreshLabel({ text, className = '', onRefresh }: RefreshLabelProps) {
  return (
    <button
      type='button'
      className={`section-label section-label-refresh ${className}`.trim()}
      onClick={onRefresh}
      title='Refresh displays and speakers'>
      {text}
      <span className='refresh-icon' aria-hidden='true'>
        &#8635;
      </span>
    </button>
  );
}
