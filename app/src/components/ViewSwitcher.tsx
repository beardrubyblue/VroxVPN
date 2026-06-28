export type Page = "shield" | "stats" | "settings" | "nodes";

const TABS: { id: Page; label: string }[] = [
  { id: "shield", label: "Shield" },
  { id: "stats", label: "Stats" },
  { id: "settings", label: "Settings" },
];

interface ViewSwitcherProps {
  page: Page;
  onChange: (page: Page) => void;
}

// BottomTabs (порт из дизайна) — текстовое меню без иконок, активный
// пункт белый + подчёркнут, неактивные приглушены с короткой чёрточкой.
export function ViewSwitcher({ page, onChange }: ViewSwitcherProps) {
  return (
    <div
      style={{
        display: "flex",
        padding: "14px 20px calc(26px + env(safe-area-inset-bottom, 0px))",
        borderTop: "1px solid var(--line)",
        background: "var(--bg)",
        flexShrink: 0,
      }}
    >
      {TABS.map((t) => {
        const active = page === t.id;
        return (
          <button
            key={t.id}
            onClick={() => onChange(t.id)}
            style={{
              flex: 1,
              padding: "6px 0",
              border: "none",
              background: "transparent",
              cursor: "pointer",
              display: "flex",
              flexDirection: "column",
              alignItems: "center",
              gap: 5,
              color: active ? "var(--fg)" : "var(--fg-dim)",
            }}
          >
            <span
              className="mono"
              style={{
                fontSize: 9,
                letterSpacing: "0.22em",
                textTransform: "uppercase",
                fontWeight: active ? 600 : 400,
              }}
            >
              {t.label}
            </span>
            <span
              style={{
                width: active ? 18 : 4,
                height: 2,
                borderRadius: 2,
                background: active ? "var(--fg)" : "var(--fg-dim)",
                opacity: active ? 1 : 0.25,
                transition: "all 0.3s",
              }}
            />
          </button>
        );
      })}
    </div>
  );
}
