interface DarkModeToggleProps {
  isDarkMode: boolean;
  onChange: (enabled: boolean) => void;
}

/** Dark/Light mode toggle buttons. */
export default function DarkModeToggle({
  isDarkMode,
  onChange,
}: DarkModeToggleProps) {
  return (
    <div className="dark-mode-toggle">
      <button
        className={`dark-mode-btn ${isDarkMode ? "active" : ""}`}
        onClick={() => onChange(true)}
      >
        <span className="icon">{"\uD83C\uDF19"}</span>
        DARK
      </button>
      <button
        className={`dark-mode-btn ${!isDarkMode ? "active" : ""}`}
        onClick={() => onChange(false)}
      >
        <span className="icon">{"\u2600\uFE0F"}</span>
        LIGHT
      </button>
    </div>
  );
}
