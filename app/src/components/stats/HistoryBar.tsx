interface HistoryBarProps {
  heightPercent: number;
  isToday: boolean;
  isSelected: boolean;
}

export function HistoryBar({ heightPercent, isToday, isSelected }: HistoryBarProps) {
  const className = ["history-bar", isToday && "today", isSelected && "selected"].filter(Boolean).join(" ");
  return <div className={className} style={{ height: `${heightPercent}%` }} />;
}
