import { useState } from "react";
import { AddNodeSheet } from "@/components/add-sheet";
import { NodesFilters, NodesHeader, NodesTabEnum, SubscriptionGroup, WireGuardGroup, WireGuardSheet, filterNodes } from "@/components/nodes";
import { SubscriptionSheet } from "@/components/subscription-sheet";
import { useAddNode, useSubscriptionActions, useWireGuardActions } from "@/hooks";
import type { useConnection, useSubscriptions, useWireGuard } from "@/hooks";
import type { Server } from "@/types";
import { isLinux } from "@/utils/platform";

type TPushToast = (text: string, kind?: "error" | "info") => void;

// ключ группы WireGuard в наборе свёрнутых (у подписок — их url)
const WIREGUARD_GROUP_KEY = "wireguard";

interface NodesScreenProps {
  subs: ReturnType<typeof useSubscriptions>;
  wireguard: ReturnType<typeof useWireGuard>;
  connection: ReturnType<typeof useConnection>;
  pushToast: TPushToast;
  onPick: (server: Server) => void;
  onBack: () => void;
}

// Список нод: группы подписок (hysteria2) с обновлением и меню, плюс группа
// импортированных WireGuard/AmneziaWG-конфигов с меню у каждого узла.
export function NodesScreen({ subs, wireguard, connection, pushToast, onPick, onBack }: NodesScreenProps) {
  const [query, setQuery] = useState("");
  const [tab, setTab] = useState<NodesTabEnum>(NodesTabEnum.All);
  const [collapsed, setCollapsed] = useState<ReadonlySet<string>>(new Set());
  const add = useAddNode(subs, wireguard, pushToast);
  const actions = useSubscriptionActions({ subs, connection, pushToast });
  const wireguardActions = useWireGuardActions({ wireguard, connection });

  const isSearching = query.trim() !== "";
  const groups = subs.subscriptions
    .map((subscription) => ({ subscription, servers: filterNodes(subscription.servers, subscription.pings, query, tab) }))
    .filter((group) => !isSearching || group.servers.length > 0);
  const wireguardServers = filterNodes(wireguard.servers, wireguard.pings, query, tab);
  const hasWireGuardGroup = wireguardServers.length > 0 || (!isSearching && wireguard.servers.length > 0);
  const subscriptionNodes = subs.subscriptions.reduce((sum, subscription) => sum + subscription.servers.length, 0);
  const hasNothing = subs.subscriptions.length === 0 && wireguard.servers.length === 0;

  function toggleGroup(key: string) {
    setCollapsed((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  }

  // во время поиска группы раскрыты — иначе совпадения прятались бы
  const isGroupOpen = (key: string) => isSearching || !collapsed.has(key);

  return (
    <div className="vrox-screen">
      <NodesHeader
        subscriptionCount={subs.subscriptions.length}
        nodeCount={subscriptionNodes + wireguard.servers.length}
        isRefreshing={subs.subscriptions.some((subscription) => subscription.refreshing)}
        onBack={onBack}
        onPaste={add.subscriptionForm.paste}
        onRefreshAll={subs.refreshAll}
        onAdd={add.open}
      />
      <NodesFilters query={query} onQueryChange={setQuery} tab={tab} onTabChange={setTab} />

      <div className="scrollable nodes-list">
        {groups.length === 0 && !hasWireGuardGroup && (
          <div className="nodes-empty">{hasNothing ? "Add a subscription or a WireGuard config" : "Nothing matches your search"}</div>
        )}
        {groups.map(({ subscription, servers }) => (
          <SubscriptionGroup
            key={subscription.url}
            subscription={subscription}
            servers={servers}
            isOpen={isGroupOpen(subscription.url)}
            activeName={connection.selectedServer?.name}
            onToggle={() => toggleGroup(subscription.url)}
            onRefresh={() => subs.refresh(subscription.url)}
            onMenu={() => actions.openMenu(subscription.url)}
            onPick={onPick}
          />
        ))}
        {hasWireGuardGroup && (
          <WireGuardGroup
            servers={wireguardServers}
            pings={wireguard.pings}
            isOpen={isGroupOpen(WIREGUARD_GROUP_KEY)}
            activeName={connection.selectedServer?.name}
            onToggle={() => toggleGroup(WIREGUARD_GROUP_KEY)}
            onPick={onPick}
            onMenu={wireguardActions.openMenu}
          />
        )}
      </div>

      <AddNodeSheet
        isOpen={add.sheet.open}
        isVisible={add.sheet.visible}
        tab={add.tab}
        onTabChange={add.setTab}
        canAddWireGuard={!isLinux}
        subscriptionForm={add.subscriptionForm}
        wireguardForm={add.wireguardForm}
        onClose={add.sheet.hide}
      />
      <SubscriptionSheet actions={actions} />
      <WireGuardSheet actions={wireguardActions} />
    </div>
  );
}
