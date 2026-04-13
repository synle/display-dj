interface HeaderProps {
  version: string;
  expanded: boolean;
  onToggle: () => void;
  onSettingsToggle: () => void;
  settingsOpen: boolean;
}

/** App header with version display, settings gear button, and expand/collapse toggle. */
export default function Header({
  version,
  expanded,
  onToggle,
  onSettingsToggle,
  settingsOpen,
}: HeaderProps) {
  return (
    <div className="header">
      <div>
        <span className="header-title">Display DJ</span>
        {version && <span className="header-version">v{version}</span>}
      </div>
      <div className="header-actions">
        <button
          className={`header-toggle ${settingsOpen ? "active" : ""}`}
          onClick={onSettingsToggle}
          title="Settings"
        >
          &#9881;
        </button>
        <button
          className="header-toggle"
          onClick={onToggle}
          title={
            expanded
              ? "Show all monitors control"
              : "Show individual monitors"
          }
        >
          <span className={`chevron ${expanded ? "expanded" : ""}`}>
            &#9662;
          </span>
        </button>
      </div>
    </div>
  );
}
