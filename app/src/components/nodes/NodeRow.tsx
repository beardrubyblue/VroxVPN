import { FlagDot } from "@/design/brand";
import { countryCodeFromName } from "@/design/country";
import type { PingResult, Server } from "@/types";
import { protocolLabel } from "@/utils/protocol";

const BAR_COUNT = 5;
// Пороги пинга (мс) для полосок сигнала: меньше пинг → больше полос.
const BAR_THRESHOLDS_MS = [40, 70, 110, 170];

function signalBars(latencyMs: number | null): number {
  if (latencyMs === null) return 0;
  const slower = BAR_THRESHOLDS_MS.filter((threshold) => latencyMs >= threshold).length;
  return BAR_COUNT - slower;
}

interface NodeRowProps {
  server: Server;
  ping: PingResult | undefined;
  isActive: boolean;
  onPick: (server: Server) => void;
}

export function NodeRow({ server, ping, isActive, onPick }: NodeRowProps) {
  const latencyMs = ping?.latency_ms ?? null;
  const bars = signalBars(latencyMs);

  return (
    <button className={isActive ? "node-row active" : "node-row"} onClick={() => onPick(server)}>
      <FlagDot code={countryCodeFromName(server.name)} size={32} />
      <div className="node-body">
        <div className="node-name">{server.name}</div>
        <div className="node-host mono">{protocolLabel(server)}</div>
      </div>
      <div className="node-ping mono" aria-label={latencyMs === null ? "No ping" : `${latencyMs} ms`}>
        {latencyMs === null ? "—" : latencyMs}
        {latencyMs !== null && <small>ms</small>}
        <div className="node-bars" aria-hidden="true">
          {Array.from({ length: BAR_COUNT }, (_, index) => (
            <div key={index} className={index < bars ? "on" : undefined} style={{ height: 3 + index * 2 }} />
          ))}
        </div>
      </div>
    </button>
  );
}
