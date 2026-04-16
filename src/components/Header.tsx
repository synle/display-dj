interface HeaderProps {
  version: string;
  onSettingsToggle: () => void;
  settingsOpen: boolean;
}

/** App header with version display and settings gear button. */
export default function Header({ version, onSettingsToggle, settingsOpen }: HeaderProps) {
  return (
    <div className='header'>
      <div>
        <span className='header-title'>Display DJ</span>
        {version && <span className='header-version'>v{version}</span>}
      </div>
      <div className='header-actions'>
        <button
          className={`header-toggle ${settingsOpen ? 'active' : ''}`}
          onClick={onSettingsToggle}
          title='Settings'>
          &#9881;
        </button>
      </div>
    </div>
  );
}
