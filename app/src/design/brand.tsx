// Логотип и брендовые марки VroxVPN (порт brand.jsx из дизайна).

export function VroxLogo({ size = 28, color = "currentColor" }: { size?: number; color?: string }) {
  // Абстрактная «V» — две вложенные линии (как в иконке приложения):
  // основная белая + приглушённая вторая.
  return (
    <svg width={size} height={size} viewBox="0 0 32 32" fill="none">
      <path d="M4 6 L16 26 L28 6" stroke={color} strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round" />
      <path
        d="M10 6 L16 16 L22 6"
        stroke={color}
        strokeWidth="2.5"
        strokeLinecap="round"
        strokeLinejoin="round"
        opacity="0.4"
      />
    </svg>
  );
}

export function VroxWordmark({ size = 18, color = "currentColor" }: { size?: number; color?: string }) {
  return (
    <div
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: 8,
        fontFamily: "var(--font-display)",
        fontWeight: 600,
        fontSize: size,
        letterSpacing: "-0.02em",
        color,
      }}
    >
      <VroxLogo size={size * 1.1} color={color} />
      <span>
        vrox<span style={{ opacity: 0.4 }}>.vpn</span>
      </span>
    </div>
  );
}

// Двухтоновый кружок-«флаг» по коду страны (порт FlagDot). Палитра
// монохромная — оттенки серого, как в дизайне.
const FLAG_PALETTES: Record<string, [string, string]> = {
  NL: ["#ffffff", "#d0d0d0"],
  DE: ["#2c2c2a", "#737371"],
  JP: ["#fafafa", "#d4d4d2"],
  US: ["#1a1a18", "#525250"],
  SE: ["#e8e8e6", "#a8a8a6"],
  CH: ["#ffffff", "#2c2c2a"],
  SG: ["#d4d4d2", "#737371"],
  FR: ["#2c2c2a", "#a8a8a6"],
  IS: ["#fafafa", "#606060"],
  UK: ["#1a1a18", "#a8a8a6"],
  CA: ["#e8e8e6", "#525250"],
  AU: ["#737371", "#1a1a18"],
};

export function FlagDot({ code, size = 24 }: { code: string; size?: number }) {
  const [c1, c2] = FLAG_PALETTES[code] || ["#888888", "#444444"];
  return (
    <div
      style={{
        width: size,
        height: size,
        borderRadius: "50%",
        overflow: "hidden",
        position: "relative",
        flexShrink: 0,
        border: "0.5px solid rgba(0,0,0,0.1)",
      }}
    >
      <div style={{ position: "absolute", inset: 0, background: c1 }} />
      <div style={{ position: "absolute", right: 0, top: 0, bottom: 0, width: "50%", background: c2 }} />
    </div>
  );
}
