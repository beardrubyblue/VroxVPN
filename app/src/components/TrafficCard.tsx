import { formatBytes, formatSpeed } from "@/utils/format";
import type { TrafficDisplay } from "@/types";

interface TrafficCardProps {
  traffic: TrafficDisplay;
}

export function TrafficCard({ traffic }: TrafficCardProps) {
  return (
    <div className="card traffic-card">
      <div className="traffic-row">
        <span className="traffic-label">↑ Отдано</span>
        <span className="traffic-speed">{formatSpeed(traffic.upSpeed)}</span>
        <span className="traffic-total">{formatBytes(traffic.totalUp)}</span>
      </div>
      <div className="traffic-row">
        <span className="traffic-label">↓ Получено</span>
        <span className="traffic-speed">{formatSpeed(traffic.downSpeed)}</span>
        <span className="traffic-total">{formatBytes(traffic.totalDown)}</span>
      </div>
    </div>
  );
}
