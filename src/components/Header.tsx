interface HeaderProps {
  version: string;
  onSettingsToggle: () => void;
  settingsOpen: boolean;
}

/** App header with version display and settings gear button. */
export default function Header({ version, onSettingsToggle, settingsOpen }: HeaderProps) {
  // Split version into base (e.g. "6.3.19") and tag (e.g. "[DEV - 04/23/2026 08:30]")
  const tagMatch = version.match(/^([\d.]+)\s*(\[.+\])$/);
  const baseVersion = tagMatch ? tagMatch[1] : version;
  const devTag = tagMatch ? tagMatch[2] : '';

  return (
    <div className='header'>
      <div>
        <span className='header-title'>Display DJ</span>
        {version && (
          <span className='header-version'>
            v{baseVersion}
            {devTag && <span className='header-dev-tag'> {devTag}</span>}
          </span>
        )}
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
