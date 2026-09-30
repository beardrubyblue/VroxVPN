import { TrafficHistoryCard } from "@/components/stats";
import { useTrafficHistory } from "@/hooks/useTrafficHistory";
import type { MemoryDebug, TrafficDisplay } from "@/types";
import { formatBytes, formatHandshakeAge, formatSpeed, MEMORY_BUDGET_BYTES } from "@/utils/format";

interface StatsScreenProps {
  connected: boolean;
  traffic: TrafficDisplay | null;
  memoryBytes: number;
  memoryDebug: MemoryDebug | null;
}

// ScreenStats (порт из дизайна, секция 03) — экран «Traffic.». Карточка
// истории трафика по дням за месяц (раньше тут был декоративный бар-чарт
// из синусоиды — реальных данных за ним не было), сетка метрик текущей
// сессии из реальных счётчиков, плюс блок памяти тоннеля.
export function StatsScreen({ connected, traffic, memoryBytes, memoryDebug }: StatsScreenProps) {
  const history = useTrafficHistory();
  const memPct = Math.min(100, (memoryBytes / MEMORY_BUDGET_BYTES) * 100);

  const metrics: [string, string, string][] = [
    ["BYTES UP", traffic ? formatBytes(traffic.totalUp) : "—", "↑ up"],
    ["BYTES DOWN", traffic ? formatBytes(traffic.totalDown) : "—", "↓ down"],
    ["UP SPEED", traffic ? formatSpeed(traffic.upSpeed) : "—", "now"],
    ["DOWN SPEED", traffic ? formatSpeed(traffic.downSpeed) : "—", "now"],
  ];

  return (
    <div className="vrox-screen">
      <div style={{ padding: "var(--screen-top) 20px 8px" }}>
        <h2 className="display" style={{ fontSize: 32, fontWeight: 600, margin: 0, letterSpacing: "-0.02em" }}>
          Traffic.
        </h2>
      </div>

      <div className="scrollable" style={{ flex: 1, padding: "16px 20px 20px" }}>
        <TrafficHistoryCard days={history.days} isLoaded={history.isLoaded} isConnected={connected} />

        {/* Сетка метрик 2×2 — текущая сессия (реальные счётчики) */}
        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 10, marginTop: 10 }}>
          {metrics.map(([k, v, s]) => (
            <div key={k} style={{ background: "var(--bg-elev-1)", border: "1px solid var(--line)", borderRadius: 16, padding: 14 }}>
              <div className="mono" style={{ fontSize: 9, color: "var(--fg-dim)", letterSpacing: "0.2em" }}>{k}</div>
              <div className="mono" style={{ fontSize: 20, marginTop: 6, letterSpacing: "-0.02em" }}>{v}</div>
              <div className="mono" style={{ fontSize: 10, color: "var(--fg-dim)", marginTop: 4 }}>{s}</div>
            </div>
          ))}
        </div>

        {/* Память тоннеля (iOS-диагностика) */}
        <div className="mono" style={{ fontSize: 10, color: "var(--fg-dim)", letterSpacing: "0.2em", marginTop: 20, marginBottom: 8 }}>
          TUNNEL MEMORY
        </div>
        <div style={{ background: "var(--bg-elev-1)", border: "1px solid var(--line)", borderRadius: 16, padding: 16 }}>
          <div style={{ display: "flex", justifyContent: "space-between", alignItems: "baseline" }}>
            <span className="mono" style={{ fontSize: 11, color: "var(--fg-dim)", letterSpacing: "0.12em" }}>RSS</span>
            <span className="mono" style={{ fontSize: 15 }}>
              {formatBytes(memoryBytes)} <span style={{ color: "var(--fg-dim)", fontSize: 11 }}>/ {formatBytes(MEMORY_BUDGET_BYTES)}</span>
            </span>
          </div>
          <div style={{ height: 5, borderRadius: 3, background: "var(--line)", overflow: "hidden", marginTop: 10 }}>
            <div
              style={{
                height: "100%",
                borderRadius: 3,
                width: `${memPct}%`,
                background: memPct > 80 ? "var(--danger)" : "var(--fg)",
                transition: "width 0.3s ease",
              }}
            />
          </div>
          {memoryDebug && (
            <div style={{ marginTop: 14, display: "flex", flexDirection: "column", gap: 7 }}>
              {(
                [
                  ["Go heap (live)", formatBytes(memoryDebug.heap_in_use)],
                  ["Non-Go (Swift/QUIC)", formatBytes(Math.max(0, memoryBytes - memoryDebug.heap_sys))],
                  ["Goroutines", String(memoryDebug.goroutines)],
                  ["Connections TCP / UDP", `${memoryDebug.tcp_relays} / ${memoryDebug.udp_relays}`],
                  // только WireGuard: «подключено, но сервер не отвечает» видно сразу
                  ...(memoryDebug.handshake_age_sec !== undefined
                    ? [["WireGuard handshake", formatHandshakeAge(memoryDebug.handshake_age_sec)]]
                    : []),
                  ...(memoryDebug.wg_error ? [["WireGuard error", memoryDebug.wg_error]] : []),
                ] as [string, string][]
              ).map(([k, v]) => (
                <div key={k} style={{ display: "flex", justifyContent: "space-between", fontSize: 12 }}>
                  <span style={{ color: "var(--fg-dim)" }}>{k}</span>
                  <span className="mono" style={{ color: "var(--fg)" }}>{v}</span>
                </div>
              ))}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
