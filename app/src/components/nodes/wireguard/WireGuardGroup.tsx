import { Ic } from "@/design/icons";
import type { PingResult, Server } from "@/types";
import { WireGuardRow } from "./WireGuardRow";

interface WireGuardGroupProps {
  servers: Server[];
  pings: Record<string, PingResult>;
  isOpen: boolean;
  activeName: string | undefined;
  onToggle: () => void;
  onPick: (server: Server) => void;
  onMenu: (id: string) => void;
}

// Группа импортированных WireGuard/AmneziaWG-конфигов на экране Nodes.
// servers — уже отфильтрованы поиском (filter-nodes.ts).
export function WireGuardGroup({ servers, pings, isOpen, activeName, onToggle, onPick, onMenu }: WireGuardGroupProps) {
  return (
    <section className="group" aria-label="WireGuard">
      <div className="group-head">
        <button className="group-toggle" onClick={onToggle} aria-expanded={isOpen}>
          <span className={isOpen ? "group-chevron open" : "group-chevron"} aria-hidden="true">
            <Ic.chevron s={14} sw={2} />
          </span>
          <span className="group-info">
            <span className="group-title mono">WireGuard</span>
            <span className="group-meta mono">{servers.length} CONFIGS</span>
          </span>
        </button>
      </div>
      {isOpen &&
        servers.map((server) => (
          <WireGuardRow
            key={server.raw_uri}
            server={server}
            ping={pings[server.name]}
            isActive={server.name === activeName}
            onPick={onPick}
            onMenu={onMenu}
          />
        ))}
    </section>
  );
}
