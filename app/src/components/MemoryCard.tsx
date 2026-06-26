import { formatBytes, MEMORY_BUDGET_BYTES } from "@/utils/format";

interface MemoryCardProps {
  memoryBytes: number;
}

// Карточка видна всегда, не только при подключении (явный запрос —
// следить за бюджетом памяти независимо от состояния VPN). При
// отключённом тоннеле — честные "0 Б", не пустое место и не
// устаревшее значение с прошлого сеанса (см. App.tsx::displayedMemoryBytes).
export function MemoryCard({ memoryBytes }: MemoryCardProps) {
  const pct = Math.min(100, (memoryBytes / MEMORY_BUDGET_BYTES) * 100);
  const level = memoryBytes > MEMORY_BUDGET_BYTES ? "danger" : pct > 70 ? "warn" : "ok";

  return (
    <div className="card memory-card">
      <div className="memory-row">
        <span className="memory-label">Память тоннеля</span>
        <span className={`memory-value memory-${level}`}>
          {formatBytes(memoryBytes)} / {formatBytes(MEMORY_BUDGET_BYTES)}
        </span>
      </div>
      <div className="memory-bar-track">
        <div className={`memory-bar-fill memory-${level}`} style={{ width: `${pct}%` }} />
      </div>
    </div>
  );
}
