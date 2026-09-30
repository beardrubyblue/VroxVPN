import { Ic } from "@/design/icons";
import type { PingResult, Server } from "@/types";
import { ManualRow } from "./ManualRow";

interface ManualGroupProps {
  servers: Server[];
  pings: Record<string, PingResult>;
  isOpen: boolean;
  activeName: string | undefined;
  onToggle: () => void;
  onPick: (server: Server) => void;
  onMenu: (id: string) => void;
}

// Группа «Added manually» — узлы, добавленные вручную (ссылка hysteria2://
// или .conf WireGuard). Группа — источник, протокол виден в строке узла.
// servers — уже отфильтрованы поиском (filter-nodes.ts).
export function ManualGroup({ servers, pings, isOpen, activeName, onToggle, onPick, onMenu }: ManualGroupProps) {
  return (
    <section className="group" aria-label="Added manually">
      <div className="group-head">
        <button className="group-toggle" onClick={onToggle} aria-expanded={isOpen}>
          <span className={isOpen ? "group-chevron open" : "group-chevron"} aria-hidden="true">
            <Ic.chevron s={14} sw={2} />
          </span>
          <span className="group-info">
            <span className="group-title mono">Added manually</span>
            <span className="group-meta mono">{servers.length} NODES</span>
          </span>
        </button>
      </div>
      {isOpen &&
        servers.map((server) => (
          <ManualRow
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
