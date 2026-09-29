import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
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
  useSubscriptions,
  useToast,
  useTrafficStats,
  useAppBootstrap,
} from "@/hooks";
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
  useAppBootstrap({ settings, subs, setSelectedServer: connection.setSelectedServer });

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
          subs={subs}
          connection={connection}
          pushToast={pushToast}
          onPick={(server) => {
            connection.setSelectedServer(server);
            setPage("shield");
          }}
          onBack={() => setPage("shield")}
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
    </div>
  );
}

export default App;
