import { NodeRow } from "@/components/nodes/NodeRow";
import type { Server, Subscription } from "@/types";
import { SubscriptionHeader } from "./SubscriptionHeader";

interface SubscriptionGroupProps {
  subscription: Subscription;
  servers: Server[];
  isOpen: boolean;
  activeName: string | undefined;
  onToggle: () => void;
  onRefresh: () => void;
  onMenu: () => void;
  onPick: (server: Server) => void;
}

// Группа серверов одной подписки. servers — уже отфильтрованный и
// отсортированный список (filter-nodes.ts); ключ строки — raw_uri, а не
// имя: у подписок бывают серверы с одинаковыми именами («NL Hysteria2»
// дважды), и ключ по имени путал React при перерисовке.
export function SubscriptionGroup({
  subscription,
  servers,
  isOpen,
  activeName,
  onToggle,
  onRefresh,
  onMenu,
  onPick,
}: SubscriptionGroupProps) {
  return (
    <section className="group" aria-label={subscription.name}>
      <SubscriptionHeader
        subscription={subscription}
        isOpen={isOpen}
        onToggle={onToggle}
        onRefresh={onRefresh}
        onMenu={onMenu}
      />
      {isOpen &&
        servers.map((server, index) => (
          <NodeRow
            key={`${server.raw_uri}-${index}`}
            server={server}
            ping={subscription.pings[server.name]}
            isActive={server.name === activeName}
            onPick={onPick}
          />
        ))}
    </section>
  );
}
