// Иконки VroxVPN — stroke-based, монохром (порт icons.jsx из дизайна).
// Каждая принимает s (size) и sw (strokeWidth).
interface IconProps {
  s?: number;
  sw?: number;
}

function base(s = 20) {
  return { width: s, height: s, viewBox: "0 0 24 24", fill: "none" as const };
}
const stroke = {
  stroke: "currentColor",
  strokeLinecap: "round" as const,
  strokeLinejoin: "round" as const,
};

export const Ic = {
  shield: ({ s, sw }: IconProps = {}) => (
    <svg {...base(s)} {...stroke} strokeWidth={sw || 1.5}>
      <path d="M12 2L4 5v7c0 5 3.5 8.5 8 10 4.5-1.5 8-5 8-10V5l-8-3z" />
    </svg>
  ),
  lock: ({ s, sw }: IconProps = {}) => (
    <svg {...base(s)} {...stroke} strokeWidth={sw || 1.5}>
      <rect x="4" y="10" width="16" height="11" rx="2" />
      <path d="M8 10V7a4 4 0 018 0v3" />
    </svg>
  ),
  globe: ({ s, sw }: IconProps = {}) => (
    <svg {...base(s)} {...stroke} strokeWidth={sw || 1.5}>
      <circle cx="12" cy="12" r="9" />
      <path d="M3 12h18M12 3c3 3 3 15 0 18M12 3c-3 3-3 15 0 18" />
    </svg>
  ),
  power: ({ s, sw }: IconProps = {}) => (
    <svg {...base(s)} {...stroke} strokeWidth={sw || 2}>
      <path d="M12 3v9" />
      <path d="M6 7a8 8 0 1012 0" />
    </svg>
  ),
  arrowRight: ({ s, sw }: IconProps = {}) => (
    <svg {...base(s)} {...stroke} strokeWidth={sw || 1.5}>
      <path d="M5 12h14M13 6l6 6-6 6" />
    </svg>
  ),
  chevron: ({ s, sw }: IconProps = {}) => (
    <svg {...base(s)} {...stroke} strokeWidth={sw || 1.5}>
      <path d="M9 6l6 6-6 6" />
    </svg>
  ),
  check: ({ s, sw }: IconProps = {}) => (
    <svg {...base(s)} {...stroke} strokeWidth={sw || 2}>
      <path d="M5 12l5 5 9-10" />
    </svg>
  ),
  alert: ({ s, sw }: IconProps = {}) => (
    <svg {...base(s)} {...stroke} strokeWidth={sw || 2}>
      <path d="M12 7v6M12 17h.01" />
    </svg>
  ),
  x: ({ s, sw }: IconProps = {}) => (
    <svg {...base(s)} {...stroke} strokeWidth={sw || 1.8}>
      <path d="M6 6l12 12M18 6L6 18" />
    </svg>
  ),
  plus: ({ s, sw }: IconProps = {}) => (
    <svg {...base(s)} {...stroke} strokeWidth={sw || 1.8}>
      <path d="M12 5v14M5 12h14" />
    </svg>
  ),
  trash: ({ s, sw }: IconProps = {}) => (
    <svg {...base(s)} {...stroke} strokeWidth={sw || 1.5}>
      <path d="M4 7h16M9 7V5a1 1 0 011-1h4a1 1 0 011 1v2M6 7l1 13a1 1 0 001 1h8a1 1 0 001-1l1-13" />
    </svg>
  ),
  sun: ({ s, sw }: IconProps = {}) => (
    <svg {...base(s)} {...stroke} strokeWidth={sw || 1.5}>
      <circle cx="12" cy="12" r="4" />
      <path d="M12 2v2M12 20v2M4 12H2M22 12h-2M5 5l1.4 1.4M17.6 17.6L19 19M5 19l1.4-1.4M17.6 6.4L19 5" />
    </svg>
  ),
  moon: ({ s, sw }: IconProps = {}) => (
    <svg {...base(s)} {...stroke} strokeWidth={sw || 1.5}>
      <path d="M21 12.8A9 9 0 1111.2 3 7 7 0 0021 12.8z" />
    </svg>
  ),
  download: ({ s, sw }: IconProps = {}) => (
    <svg {...base(s)} {...stroke} strokeWidth={sw || 1.5}>
      <path d="M12 3v12M6 11l6 6 6-6M4 20h16" />
    </svg>
  ),
  more: ({ s, sw }: IconProps = {}) => (
    <svg {...base(s)} {...stroke} strokeWidth={sw || 2.2}>
      <path d="M6 12h.01M12 12h.01M18 12h.01" />
    </svg>
  ),
  edit: ({ s, sw }: IconProps = {}) => (
    <svg {...base(s)} {...stroke} strokeWidth={sw || 1.5}>
      <path d="M4 20h4L19 9l-4-4L4 16v4zM13 7l4 4" />
    </svg>
  ),
  copy: ({ s, sw }: IconProps = {}) => (
    <svg {...base(s)} {...stroke} strokeWidth={sw || 1.5}>
      <rect x="9" y="9" width="11" height="11" rx="2" />
      <path d="M5 15V6a2 2 0 012-2h9" />
    </svg>
  ),
  refresh: ({ s, sw }: IconProps = {}) => (
    <svg {...base(s)} {...stroke} strokeWidth={sw || 1.5}>
      <path d="M3 12a9 9 0 0115-6.7L21 8M21 3v5h-5M21 12a9 9 0 01-15 6.7L3 16M3 21v-5h5" />
    </svg>
  ),
};
