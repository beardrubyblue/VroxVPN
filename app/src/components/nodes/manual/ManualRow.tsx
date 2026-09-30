import { NodeRow } from "@/components/nodes/NodeRow";
import { Ic } from "@/design/icons";
import type { PingResult, Server } from "@/types";

interface ManualRowProps {
  server: Server;
  ping: PingResult | undefined;
  isActive: boolean;
  onPick: (server: Server) => void;
  onMenu: (id: string) => void;
}

// Строка узла, добавленного вручную: у каждого своё меню (переименовать /
// удалить) — у подписок меню на всю группу.
export function ManualRow({ server, ping, isActive, onPick, onMenu }: ManualRowProps) {
  const id = server.manual_id ?? "";
  return (
    <div className="manual-row">
      <NodeRow server={server} ping={ping} isActive={isActive} onPick={onPick} />
      <button className="round-btn small" onClick={() => onMenu(id)} aria-label={`${server.name} options`}>
        <Ic.more s={16} />
      </button>
    </div>
  );
}
