import { useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { readText } from "@tauri-apps/plugin-clipboard-manager";
import { AddSubscriptionSheet } from "@/components/AddSubscriptionSheet";
import { ShieldScreen } from "@/components/screens/ShieldScreen";
import { NodesScreen } from "@/components/screens/NodesScreen";
import { StatsScreen } from "@/components/screens/StatsScreen";
import { SettingsScreen, type Theme } from "@/components/screens/SettingsScreen";
import { ToastBanner } from "@/components/ToastBanner";
import { ViewSwitcher, type Page } from "@/components/ViewSwitcher";
import {
  useAppUpdate,
  useConnection,
  useGeoUpdates,
  useSettings,
  useSheet,
  useSubscriptions,
  useToast,
  useTrafficStats,
  useAppBootstrap,
} from "@/hooks";
import type { PingResult } from "@/types";
import "./App.css";

function App() {
  const [page, setPage] = useState<Page>("shield");
  const [theme, setTheme] = useState<Theme>("dark");

  const { toast, pushToast } = useToast();
  const subs = useSubscriptions(pushToast);
  const settings = useSettings();
  const connection = useConnection({
    subscriptions: subs.subscriptions,
    subscriptionsRef: subs.subscriptionsRef,
    ruBypass: settings.ruBypass,
    killSwitch: settings.killSwitch,
    pushToast,
  });
  const { traffic, memoryBytes, memoryDebug } = useTrafficStats(connection.status.connected, pushToast);
  const update = useAppUpdate(pushToast);
  const geo = useGeoUpdates(pushToast);
  const addSheet = useSheet();

  const [newUrl, setNewUrl] = useState("");
  const [addError, setAddError] = useState("");

  useAppBootstrap({ settings, subs, setSelectedServer: connection.setSelectedServer });

  // Плоский список серверов со всех подписок + объединённая карта пингов
  const allServers = useMemo(() => subs.subscriptions.flatMap((s) => s.servers), [subs.subscriptions]);
  const allPings = useMemo(() => {
    const m: Record<string, PingResult> = {};
    for (const s of subs.subscriptions) Object.assign(m, s.pings);
    return m;
  }, [subs.subscriptions]);

  function openAddSheet() {
    setNewUrl("");
    setAddError("");
    addSheet.show();
  }

  async function confirmAddSubscription() {
    if (!newUrl.trim()) {
      setAddError("Enter a subscription URL");
      return;
    }
    setAddError("");
    if (await subs.addFromUrl(newUrl.trim())) {
      addSheet.hide();
    }
  }

  async function pasteAndAdd() {
    let text: string | null;
    try {
      text = await readText();
    } catch {
      pushToast("No clipboard access", "error");
      return;
    }
    if (!text || !text.trim()) {
      pushToast("Clipboard is empty", "error");
      return;
    }
    await subs.addFromUrl(text.trim());
  }

  const showTabs = page !== "nodes";

  return (
    <div className={`window vrox-${theme}`}>
      <ToastBanner toast={toast} />

      {page === "shield" && (
        <ShieldScreen
          connected={connection.status.connected}
          busy={connection.busy}
          server={connection.selectedServer}
          onToggle={connection.toggleConnection}
          onOpenLocations={() => setPage("nodes")}
        />
      )}

      {page === "nodes" && (
        <NodesScreen
          servers={allServers}
          pings={allPings}
          activeName={connection.selectedServer?.name}
          onPick={(server) => {
            connection.setSelectedServer(server);
            setPage("shield");
          }}
          onBack={() => setPage("shield")}
          onAdd={openAddSheet}
          onPaste={pasteAndAdd}
        />
      )}

      {page === "stats" && (
        <StatsScreen
          connected={connection.status.connected}
          traffic={traffic}
          memoryBytes={connection.status.connected ? memoryBytes : 0}
          memoryDebug={connection.status.connected ? memoryDebug : null}
        />
      )}

      {page === "settings" && (
        <SettingsScreen
          theme={theme}
          setTheme={setTheme}
          ruBypass={settings.ruBypass}
          onRuBypassChange={settings.onRuBypassChange}
          killSwitch={settings.killSwitch}
          onKillSwitchChange={settings.onKillSwitchChange}
          connected={connection.status.connected}
          geoipLoading={geo.geoipLoading}
          onUpdateGeoip={geo.updateGeoip}
          geositeLoading={geo.geositeLoading}
          onUpdateGeosite={geo.updateGeosite}
          bypassStatus={geo.bypassStatus}
          idleTimeoutSeconds={settings.idleTimeoutSeconds}
          onIdleTimeoutSecondsChange={settings.onIdleTimeoutSecondsChange}
          maxTcpConnections={settings.maxTcpConnections}
          onMaxTcpConnectionsChange={settings.onMaxTcpConnectionsChange}
          maxUdpConnections={settings.maxUdpConnections}
          onMaxUdpConnectionsChange={settings.onMaxUdpConnectionsChange}
          updateChecking={update.checking}
          onCheckUpdate={update.check}
          updateInfo={update.info}
          updateInstalling={update.installing}
          onInstallUpdate={update.install}
          onQuit={() => invoke("quit_app")}
        />
      )}

      {showTabs && <ViewSwitcher page={page} onChange={setPage} />}

      <AddSubscriptionSheet
        open={addSheet.open}
        visible={addSheet.visible}
        url={newUrl}
        onUrlChange={setNewUrl}
        error={addError}
        onConfirm={confirmAddSubscription}
        onClose={addSheet.hide}
      />
    </div>
  );
}

export default App;
