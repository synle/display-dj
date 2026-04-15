interface KeepAwakeToggleProps {
  isActive: boolean;
  onChange: (enabled: boolean) => void;
}

/** Toggle button to prevent the system from sleeping (similar to Caffeine).
 * Shows "Keep Awake" when active and "Keep Awake: Off" when inactive. */
export default function KeepAwakeToggle({ isActive, onChange }: KeepAwakeToggleProps) {
  return (
    <div className='keep-awake-toggle'>
      <button
        className={`keep-awake-btn ${isActive ? 'active' : ''}`}
        onClick={() => onChange(!isActive)}
        title={
          isActive
            ? 'System is being kept awake — click to allow sleep'
            : 'Click to prevent the system from sleeping'
        }>
        <span className='icon'>{'\u2615'}</span>
        {isActive ? 'Keep Awake' : 'Keep Awake: Off'}
      </button>
    </div>
  );
}
