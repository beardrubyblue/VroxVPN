import type { MemoryDebug } from "@/types";
import { formatBytes, MEMORY_BUDGET_BYTES } from "@/utils/format";

interface MemoryCardProps {
  memoryBytes: number;
  // Детальная разбивка — только macOS/iOS; null на Linux и до первого
  // ответа .appex (см. useTrafficStats). Когда есть — показываем КУДА
  // уходит память (Go-куча vs RSS процесса vs число relay/горутин).
  memoryDebug?: MemoryDebug | null;
}

// Карточка видна всегда, не только при подключении (явный запрос —
// следить за бюджетом памяти независимо от состояния VPN). При
// отключённом тоннеле — честные "0 Б", не пустое место и не
// устаревшее значение с прошлого сеанса (см. App.tsx::displayedMemoryBytes).
export function MemoryCard({ memoryBytes, memoryDebug }: MemoryCardProps) {
  const pct = Math.min(100, (memoryBytes / MEMORY_BUDGET_BYTES) * 100);
  const level = memoryBytes > MEMORY_BUDGET_BYTES ? "danger" : pct > 70 ? "warn" : "ok";

  // RSS всего процесса (memoryBytes) почти всегда заметно больше Go-кучи
  // (heap_in_use) — разница это Swift-рантайм, NetworkExtension.framework,
  // QUIC-буфера вне Go-кучи и страницы, которые ОС ещё не забрала назад.
  // Видеть обе цифры разом — главное: если растёт RSS, но не Go-куча,
  // проблема НЕ в нашем Go-коде (relay-горутины/буфера), а во
  // фреймворках/QUIC, и наоборот.
  const heapInUse = memoryDebug?.heap_in_use ?? 0;

  return (
    <div className="card memory-card">
      <div className="memory-row">
        <span className="memory-label">Память тоннеля (RSS)</span>
        <span className={`memory-value memory-${level}`}>
          {formatBytes(memoryBytes)} / {formatBytes(MEMORY_BUDGET_BYTES)}
        </span>
      </div>
      <div className="memory-bar-track">
        <div className={`memory-bar-fill memory-${level}`} style={{ width: `${pct}%` }} />
      </div>

      {memoryDebug && (
        <div className="memory-breakdown">
          <div className="memory-breakdown-row">
            <span>Go-куча (живая)</span>
            <span>{formatBytes(heapInUse)}</span>
          </div>
          <div className="memory-breakdown-row">
            <span>Go-рантайм (всего у ОС)</span>
            <span>{formatBytes(memoryDebug.heap_sys)}</span>
          </div>
          <div className="memory-breakdown-row">
            <span>Вне Go (Swift/QUIC/фреймворки)</span>
            <span>{formatBytes(Math.max(0, memoryBytes - memoryDebug.heap_sys))}</span>
          </div>
          <div className="memory-breakdown-row">
            <span>Горутины</span>
            <span>{memoryDebug.goroutines}</span>
          </div>
          <div className="memory-breakdown-row">
            <span>Соединения TCP / UDP</span>
            <span>
              {memoryDebug.tcp_relays} / {memoryDebug.udp_relays}
              {memoryDebug.registry_size !== memoryDebug.tcp_relays + memoryDebug.udp_relays && (
                <span className="memory-warn"> (реестр {memoryDebug.registry_size})</span>
              )}
            </span>
          </div>
          {memoryDebug.avail_mem > 0 && memoryDebug.avail_mem < 500 * 1024 * 1024 && (
            <div className="memory-breakdown-row">
              <span>iOS даёт ещё</span>
              <span>{formatBytes(memoryDebug.avail_mem)}</span>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
