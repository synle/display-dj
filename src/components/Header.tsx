interface HeaderProps {
  version: string;
  expanded: boolean;
  onToggle: () => void;
}

export default function Header({ version, expanded, onToggle }: HeaderProps) {
  return (
    <div className="header">
      <div>
        <span className="header-title">Display DJ</span>
        {version && <span className="header-version">v{version}</span>}
      </div>
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
  );
}
