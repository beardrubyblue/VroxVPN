import { useState } from "react";
import { AddSubscriptionSheet } from "@/components/AddSubscriptionSheet";
import { NodesFilters, NodesHeader, NodesTabEnum, SubscriptionGroup, filterNodes } from "@/components/nodes";
import { SubscriptionSheet } from "@/components/subscription-sheet";
import { useAddSubscription, useSubscriptionActions } from "@/hooks";
import type { useConnection, useSubscriptions } from "@/hooks";
import type { Server } from "@/types";

type TPushToast = (text: string, kind?: "error" | "info") => void;

interface NodesScreenProps {
  subs: ReturnType<typeof useSubscriptions>;
  connection: ReturnType<typeof useConnection>;
  pushToast: TPushToast;
  onPick: (server: Server) => void;
  onBack: () => void;
}

// Список нод, сгруппированный по подпискам: у каждой группы — обновление
// и меню (переименовать / скопировать ссылку / удалить). Раньше (после
// редизайна bf782fe) все подписки склеивались в один плоский список.
export function NodesScreen({ subs, connection, pushToast, onPick, onBack }: NodesScreenProps) {
  const [query, setQuery] = useState("");
  const [tab, setTab] = useState<NodesTabEnum>(NodesTabEnum.All);
  const [collapsed, setCollapsed] = useState<ReadonlySet<string>>(new Set());
  const add = useAddSubscription(subs, pushToast);
  const actions = useSubscriptionActions({ subs, connection, pushToast });

  const isSearching = query.trim() !== "";
  const groups = subs.subscriptions
    .map((subscription) => ({ subscription, servers: filterNodes(subscription.servers, subscription.pings, query, tab) }))
    .filter((group) => !isSearching || group.servers.length > 0);
  const nodeCount = subs.subscriptions.reduce((sum, subscription) => sum + subscription.servers.length, 0);

  function toggleGroup(url: string) {
    setCollapsed((prev) => {
      const next = new Set(prev);
      if (next.has(url)) next.delete(url);
      else next.add(url);
      return next;
    });
  }

  return (
    <div className="vrox-screen">
      <NodesHeader
        subscriptionCount={subs.subscriptions.length}
        nodeCount={nodeCount}
        isRefreshing={subs.subscriptions.some((subscription) => subscription.refreshing)}
        onBack={onBack}
        onPaste={add.paste}
        onRefreshAll={subs.refreshAll}
        onAdd={add.open}
      />
      <NodesFilters query={query} onQueryChange={setQuery} tab={tab} onTabChange={setTab} />

      <div className="scrollable nodes-list">
        {groups.length === 0 && (
          <div className="nodes-empty">
            {subs.subscriptions.length === 0 ? "Add a subscription to see your nodes" : "Nothing matches your search"}
          </div>
        )}
        {groups.map(({ subscription, servers }) => (
          <SubscriptionGroup
            key={subscription.url}
            subscription={subscription}
            servers={servers}
            // во время поиска группы раскрыты — иначе совпадения прятались бы
            isOpen={isSearching || !collapsed.has(subscription.url)}
            activeName={connection.selectedServer?.name}
            onToggle={() => toggleGroup(subscription.url)}
            onRefresh={() => subs.refresh(subscription.url)}
            onMenu={() => actions.openMenu(subscription.url)}
            onPick={onPick}
          />
        ))}
      </div>

      <AddSubscriptionSheet
        open={add.sheet.open}
        visible={add.sheet.visible}
        url={add.url}
        onUrlChange={add.setUrl}
        error={add.error}
        onConfirm={add.confirm}
        onClose={add.sheet.hide}
      />
      <SubscriptionSheet actions={actions} />
    </div>
  );
}
