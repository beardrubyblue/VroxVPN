import { useState } from "react";
import { formatBytes } from "@/utils/format";
import { formatDayLabel, HISTORY_DAYS } from "@/utils/traffic-history";
import type { DayTraffic } from "@/types";
import { HistoryChart } from "./HistoryChart";

interface TrafficHistoryCardProps {
  days: DayTraffic[];
  isLoaded: boolean;
  isConnected: boolean;
}

// Главная карточка экрана Stats: трафик за месяц по дням. В заголовке —
// сумма за месяц или выбранный на графике день (с разбивкой ↑/↓).
export function TrafficHistoryCard({ days, isLoaded, isConnected }: TrafficHistoryCardProps) {
  const [selectedIndex, setSelectedIndex] = useState<number | null>(null);
  const selected = selectedIndex === null ? null : days[selectedIndex];
  const isToday = selectedIndex === days.length - 1;

  const upload = selected ? selected.upload : days.reduce((sum, day) => sum + day.upload, 0);
  const download = selected ? selected.download : days.reduce((sum, day) => sum + day.download, 0);
  const label = selected ? `${formatDayLabel(selected.date)}${isToday ? " · TODAY" : ""}` : `LAST ${HISTORY_DAYS} DAYS`;
  const isEmpty = isLoaded && upload + download === 0 && selected === null;

  return (
    <div className="history-card">
      <div className="history-head">
        <div aria-live="polite">
          <div className="history-label mono">{label}</div>
          <div className="history-value display">{formatBytes(upload + download)}</div>
          <div className="history-split mono">
            ↑ {formatBytes(upload)} · ↓ {formatBytes(download)}
          </div>
        </div>
        <div className="tag">{isConnected ? "▲ LIVE" : "IDLE"}</div>
      </div>
      <HistoryChart days={days} selectedIndex={selectedIndex} onSelect={setSelectedIndex} />
      <div className="history-axis mono" aria-hidden="true">
        <span>{formatDayLabel(days[0].date)}</span>
        <span>TODAY</span>
      </div>
      {isEmpty && <p className="history-empty">Traffic will appear after the first connection</p>}
    </div>
  );
}
