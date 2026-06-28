import { useMemo } from "react";
import type { MemoryDebug, TrafficDisplay } from "@/types";
import { formatBytes, formatSpeed, MEMORY_BUDGET_BYTES } from "@/utils/format";

interface StatsScreenProps {
  connected: boolean;
  traffic: TrafficDisplay | null;
  memoryBytes: number;
  memoryDebug: MemoryDebug | null;
}

// ScreenStats (порт из дизайна, секция 03) — экран «Traffic.». Большая
// карта суммарного трафика + декоративный бар-чарт, сетка метрик из
// реальных счётчиков, плюс блок памяти тоннеля (диагностика iOS).
export function StatsScreen({ connected, traffic, memoryBytes, memoryDebug }: StatsScreenProps) {
  // Декоративный бар-чарт (как в дизайне) — детерминированный, не
  // ре-рендерится каждый кадр. Реального тайм-ряда у нас нет.
  const bars = useMemo(() => Array.from({ length: 24 }).map((_, i) => 0.2 + Math.abs(Math.sin(i * 0.7)) * 0.6), []);

  const total = traffic ? traffic.totalUp + traffic.totalDown : 0;
  const memPct = Math.min(100, (memoryBytes / MEMORY_BUDGET_BYTES) * 100);

  const metrics: [string, string, string][] = [
    ["BYTES UP", traffic ? formatBytes(traffic.totalUp) : "—", "↑ up"],
    ["BYTES DOWN", traffic ? formatBytes(traffic.totalDown) : "—", "↓ down"],
    ["UP SPEED", traffic ? formatSpeed(traffic.upSpeed) : "—", "now"],
    ["DOWN SPEED", traffic ? formatSpeed(traffic.downSpeed) : "—", "now"],
  ];

  return (
    <div className="vrox-screen">
      <div style={{ padding: "60px 20px 8px" }}>
        <div className="mono" style={{ fontSize: 11, color: "var(--fg-dim)", letterSpacing: "0.15em" }}>
          {connected ? "TUNNEL · LIVE" : "TUNNEL · IDLE"}
        </div>
        <h2 className="display" style={{ fontSize: 32, fontWeight: 600, margin: "4px 0 0", letterSpacing: "-0.02em" }}>
          Traffic.
        </h2>
      </div>

      <div className="scrollable" style={{ flex: 1, padding: "16px 20px 20px" }}>
        {/* Большая карта суммарного трафика */}
        <div style={{ background: "var(--bg-elev-1)", border: "1px solid var(--line)", borderRadius: 22, padding: 20 }}>
          <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-end" }}>
            <div>
              <div className="mono" style={{ fontSize: 10, color: "var(--fg-dim)", letterSpacing: "0.2em" }}>TOTAL · SESSION</div>
              <div className="display" style={{ fontSize: 44, fontWeight: 600, letterSpacing: "-0.03em", lineHeight: 1 }}>
                {formatBytes(total)}
              </div>
            </div>
            <div className="tag">{connected ? "▲ LIVE" : "IDLE"}</div>
          </div>
          <div style={{ display: "flex", gap: 3, marginTop: 24, alignItems: "flex-end", height: 80 }}>
            {bars.map((v, i) => (
              <div
                key={i}
                style={{
                  flex: 1,
                  height: `${v * 100}%`,
                  borderRadius: 2,
                  background: i === 18 ? "var(--fg)" : "var(--fg-dim)",
                  opacity: connected ? (i === 18 ? 1 : 0.35) : 0.15,
                }}
              />
            ))}
          </div>
        </div>

        {/* Сетка метрик 2×2 (реальные счётчики) */}
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
