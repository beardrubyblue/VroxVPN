import { NodeRow } from "@/components/nodes/NodeRow";
import { Ic } from "@/design/icons";
import type { PingResult, Server } from "@/types";

interface WireGuardRowProps {
  server: Server;
  ping: PingResult | undefined;
  isActive: boolean;
  onPick: (server: Server) => void;
  onMenu: (id: string) => void;
}

// Строка WireGuard-узла: у каждого конфига своё меню (переименовать /
// удалить) — у подписок меню на всю группу.
export function WireGuardRow({ server, ping, isActive, onPick, onMenu }: WireGuardRowProps) {
  const id = server.wireguard?.id ?? "";
  return (
    <div className="wg-row">
      <NodeRow server={server} ping={ping} isActive={isActive} onPick={onPick} />
      <button className="round-btn small" onClick={() => onMenu(id)} aria-label={`${server.name} options`}>
        <Ic.more s={16} />
      </button>
    </div>
  );
}
