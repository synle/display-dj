interface HeaderProps {
  version: string;
  onSettingsToggle: () => void;
  settingsOpen: boolean;
}

/** App header with version display and settings gear button. */
export default function Header({ version, onSettingsToggle, settingsOpen }: HeaderProps) {
  // Split version to find dev/beta tag (e.g. "[DEV - 04/23/2026 08:30]")
  // Version format: "6.3.19 [DEV - ...] (arm64)" or "6.3.19 (arm64)"
  const tagMatch = version.match(/(\[.+?\])/);
  const devTag = tagMatch ? tagMatch[1] : '';
  const baseVersion = devTag ? version.replace(devTag, '').replace(/\s+/g, ' ').trim() : version;

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
