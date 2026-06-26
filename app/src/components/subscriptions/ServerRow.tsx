import type { PingResult, Server } from "@/types";

interface ServerRowProps {
  server: Server;
  selected: boolean;
  selectable: boolean;
  ping: PingResult | undefined;
  onSelect: () => void;
  onPingError: (error: string) => void;
}

export function ServerRow({ server, selected, selectable, ping, onSelect, onPingError }: ServerRowProps) {
  return (
    <div
      className={`list-row clickable ${selected ? "selected" : ""}`}
      onClick={() => selectable && onSelect()}
    >
      <span className="row-title">{server.name}</span>
      {ping !== undefined && (
        <span
          className="muted-note"
          title={ping.error ?? undefined}
          onClick={(e) => {
            // прочерк сам по себе ничего не объясняет — раньше "сервер
            // недоступен" и "у нас сломан вызов ping" выглядели
            // одинаково, тапом показываем реальную причину текстом
            // (см. ping.rs::ping_host)
            if (!ping.error) return;
            e.stopPropagation();
            onPingError(ping.error);
          }}
        >
          {ping.latency_ms === null ? "—" : `${ping.latency_ms} мс`}
        </span>
      )}
    </div>
  );
}
