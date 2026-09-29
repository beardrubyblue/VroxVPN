import { useState } from "react";
import type { KeyboardEvent, PointerEvent } from "react";
import type { DayTraffic } from "@/types";
import { HistoryBar } from "./HistoryBar";

interface HistoryChartProps {
  days: DayTraffic[];
  selectedIndex: number | null;
  onSelect: (index: number | null) => void;
}

function dayTotal(day: DayTraffic): number {
  return day.upload + day.download;
}

// Столбики по дням. Зажать и вести пальцем/мышью — выбранный день
// показывается в заголовке карточки, отпустить — снова сумма за месяц.
// С клавиатуры: ←/→ выбирают день, Esc сбрасывает.
export function HistoryChart({ days, selectedIndex, onSelect }: HistoryChartProps) {
  const [isPressing, setIsPressing] = useState(false);
  const maxTotal = Math.max(...days.map(dayTotal), 1);
  const lastIndex = days.length - 1;

  function indexAt(event: PointerEvent<HTMLDivElement>): number {
    const rect = event.currentTarget.getBoundingClientRect();
    const ratio = (event.clientX - rect.left) / rect.width;
    return Math.min(lastIndex, Math.max(0, Math.floor(ratio * days.length)));
  }

  function onPointerDown(event: PointerEvent<HTMLDivElement>) {
    event.currentTarget.setPointerCapture(event.pointerId);
    setIsPressing(true);
    onSelect(indexAt(event));
  }

  function onPointerMove(event: PointerEvent<HTMLDivElement>) {
    if (isPressing) onSelect(indexAt(event));
  }

  function onPointerEnd() {
    setIsPressing(false);
    onSelect(null);
  }

  function onKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    // первый нажатый ←/→ выбирает сегодняшний день, дальше — шаг по дням
    if (event.key === "ArrowLeft") onSelect(selectedIndex === null ? lastIndex : Math.max(0, selectedIndex - 1));
    else if (event.key === "ArrowRight") onSelect(selectedIndex === null ? lastIndex : Math.min(lastIndex, selectedIndex + 1));
    else if (event.key === "Escape") onSelect(null);
    else return;
    event.preventDefault();
  }

  return (
    <div
      className={selectedIndex === null ? "history-chart" : "history-chart selecting"}
      role="group"
      aria-label="Traffic by day, last 30 days. Use arrow keys to pick a day."
      tabIndex={0}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerEnd}
      onPointerCancel={onPointerEnd}
      onKeyDown={onKeyDown}
      onBlur={() => onSelect(null)}
    >
      {days.map((day, index) => (
        <HistoryBar
          key={day.date}
          heightPercent={(dayTotal(day) / maxTotal) * 100}
          isToday={index === lastIndex}
          isSelected={index === selectedIndex}
        />
      ))}
    </div>
  );
}
