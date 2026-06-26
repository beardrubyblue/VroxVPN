type Page = "home" | "settings";

interface ViewSwitcherProps {
  page: Page;
  onChange: (page: Page) => void;
}

export function ViewSwitcher({ page, onChange }: ViewSwitcherProps) {
  return (
    <nav className="view-switcher">
      <button className={page === "home" ? "active" : ""} onClick={() => onChange("home")}>
        <span className="icon" aria-hidden="true">
          ⌂
        </span>
        Главная
      </button>
      <button className={page === "settings" ? "active" : ""} onClick={() => onChange("settings")}>
        <span className="icon" aria-hidden="true">
          ⚙
        </span>
        Настройки
      </button>
    </nav>
  );
}
