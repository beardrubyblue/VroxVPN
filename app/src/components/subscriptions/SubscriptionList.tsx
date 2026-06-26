import type { Server, Subscription } from "@/types";
import { SubscriptionCard } from "./SubscriptionCard";

interface SubscriptionListProps {
  subscriptions: Subscription[];
  selectedServerName: string | undefined;
  selectable: boolean;
  onRefresh: (url: string) => void;
  onPing: (url: string) => void;
  onDeleteRequest: (url: string, name: string) => void;
  onSelectServer: (server: Server) => void;
  onPingError: (error: string) => void;
}

export function SubscriptionList({
  subscriptions,
  selectedServerName,
  selectable,
  onRefresh,
  onPing,
  onDeleteRequest,
  onSelectServer,
  onPingError,
}: SubscriptionListProps) {
  if (subscriptions.length === 0) {
    return <div className="empty-placeholder">Нет подписок — добавь через кнопку «Добавить» снизу</div>;
  }

  return (
    <>
      {subscriptions.map((sub) => (
        <SubscriptionCard
          key={sub.url}
          subscription={sub}
          selectedServerName={selectedServerName}
          selectable={selectable}
          onRefresh={() => onRefresh(sub.url)}
          onPing={() => onPing(sub.url)}
          onDeleteRequest={() => onDeleteRequest(sub.url, sub.name)}
          onSelectServer={onSelectServer}
          onPingError={onPingError}
        />
      ))}
    </>
  );
}
