import { RefreshIcon, PingIcon, TrashIcon } from "@/components/icons";
import type { Server, Subscription } from "@/types";
import { ServerRow } from "./ServerRow";

interface SubscriptionCardProps {
  subscription: Subscription;
  selectedServerName: string | undefined;
  selectable: boolean;
  onRefresh: () => void;
  onPing: () => void;
  onDeleteRequest: () => void;
  onSelectServer: (server: Server) => void;
  onPingError: (error: string) => void;
}

export function SubscriptionCard({
  subscription,
  selectedServerName,
  selectable,
  onRefresh,
  onPing,
  onDeleteRequest,
  onSelectServer,
  onPingError,
}: SubscriptionCardProps) {
  return (
    <div className="card">
      <div className="list-row sub-header">
        <span className="row-title">{subscription.name}</span>
        <div className="row-actions">
          <button
            className="icon-btn"
            title="Обновить подписку"
            aria-label="Обновить подписку"
            onClick={onRefresh}
            disabled={subscription.refreshing}
          >
            {subscription.refreshing ? <span className="spinner" /> : <RefreshIcon />}
          </button>
          <button
            className="icon-btn"
            title="Проверить пинг серверов"
            aria-label="Проверить пинг серверов"
            onClick={onPing}
            disabled={subscription.pinging}
          >
            {subscription.pinging ? <span className="spinner" /> : <PingIcon />}
          </button>
          <button
            className="icon-btn destructive"
            title="Удалить подписку"
            aria-label="Удалить подписку"
            onClick={onDeleteRequest}
          >
            <TrashIcon />
          </button>
        </div>
      </div>
      {subscription.servers.map((server) => (
        <ServerRow
          key={server.name}
          server={server}
          selected={selectedServerName === server.name}
          selectable={selectable}
          ping={subscription.pings[server.name]}
          onSelect={() => onSelectServer(server)}
          onPingError={onPingError}
        />
      ))}
    </div>
  );
}
