import { useState } from "react";
import { AddNodeSheet } from "@/components/add-sheet";
import { ManualGroup, ManualSheet, NodesFilters, NodesHeader, NodesTabEnum, SubscriptionGroup, filterNodes } from "@/components/nodes";
import { SubscriptionSheet } from "@/components/subscription-sheet";
import { useAddNode, useManualActions, useSubscriptionActions } from "@/hooks";
import type { useConnection, useManualServers, useSubscriptions } from "@/hooks";
import type { Server } from "@/types";
import { isLinux } from "@/utils/platform";

type TPushToast = (text: string, kind?: "error" | "info", detail?: string) => void;

// ключ группы «Added manually» в наборе свёрнутых (у подписок — их url)
const MANUAL_GROUP_KEY = "manual";

interface NodesScreenProps {
  subs: ReturnType<typeof useSubscriptions>;
  manual: ReturnType<typeof useManualServers>;
  connection: ReturnType<typeof useConnection>;
  pushToast: TPushToast;
  onPick: (server: Server) => void;
  onBack: () => void;
}

// Список нод, сгруппированный по ИСТОЧНИКУ: группы подписок (обновление и
// меню на всю группу) и «Added manually» (меню у каждого узла). Протокол —
// свойство узла: метка в строке и фильтр Hysteria2 / WireGuard.
export function NodesScreen({ subs, manual, connection, pushToast, onPick, onBack }: NodesScreenProps) {
  const [query, setQuery] = useState("");
  const [tab, setTab] = useState<NodesTabEnum>(NodesTabEnum.All);
  const [collapsed, setCollapsed] = useState<ReadonlySet<string>>(new Set());
  const add = useAddNode(subs, manual, pushToast);
  const actions = useSubscriptionActions({ subs, connection, pushToast });
  const manualActions = useManualActions({ manual, connection });

  // при поиске или фильтре по протоколу пустые группы скрыты, а непустые раскрыты
  const isFiltering = query.trim() !== "" || tab === NodesTabEnum.Hysteria2 || tab === NodesTabEnum.WireGuard;
  const groups = subs.subscriptions
    .map((subscription) => ({ subscription, servers: filterNodes(subscription.servers, subscription.pings, query, tab) }))
    .filter((group) => !isFiltering || group.servers.length > 0);
  const manualServers = filterNodes(manual.servers, manual.pings, query, tab);
  const hasManualGroup = isFiltering ? manualServers.length > 0 : manual.servers.length > 0;
  const subscriptionNodes = subs.subscriptions.reduce((sum, subscription) => sum + subscription.servers.length, 0);
  const hasNothing = subs.subscriptions.length === 0 && manual.servers.length === 0;

  function toggleGroup(key: string) {
    setCollapsed((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  }

  const isGroupOpen = (key: string) => isFiltering || !collapsed.has(key);

  return (
    <div className="vrox-screen">
      <NodesHeader
        subscriptionCount={subs.subscriptions.length}
        nodeCount={subscriptionNodes + manual.servers.length}
        isRefreshing={subs.subscriptions.some((subscription) => subscription.refreshing)}
        onBack={onBack}
        onPaste={add.paste}
        onRefreshAll={subs.refreshAll}
        onAdd={add.open}
      />
      <NodesFilters query={query} onQueryChange={setQuery} tab={tab} onTabChange={setTab} />

      <div className="scrollable nodes-list">
        {groups.length === 0 && !hasManualGroup && (
          <div className="nodes-empty">{hasNothing ? "Add a subscription or a server" : "Nothing matches your filter"}</div>
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
        {hasManualGroup && (
          <ManualGroup
            servers={manualServers}
            pings={manual.pings}
            isOpen={isGroupOpen(MANUAL_GROUP_KEY)}
            activeName={connection.selectedServer?.name}
            onToggle={() => toggleGroup(MANUAL_GROUP_KEY)}
            onPick={onPick}
            onMenu={manualActions.openMenu}
          />
        )}
      </div>

      <AddNodeSheet
        isOpen={add.sheet.open}
        isVisible={add.sheet.visible}
        tab={add.tab}
        onTabChange={add.setTab}
        canPickFileOrQr={!isLinux}
        subscriptionForm={add.subscriptionForm}
        serverForm={add.serverForm}
        onClose={add.sheet.hide}
      />
      <SubscriptionSheet actions={actions} />
      <ManualSheet actions={manualActions} />
    </div>
  );
}
