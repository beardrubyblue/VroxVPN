import { useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ShieldScreen } from "@/components/screens/ShieldScreen";
import { NodesScreen } from "@/components/screens/NodesScreen";
import { StatsScreen } from "@/components/screens/StatsScreen";
import { SettingsScreen, type Theme } from "@/components/screens/SettingsScreen";
import { ToastStack } from "@/components/toast";
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
  useManualServers,
} from "@/hooks";
import "./App.css";

function App() {
  const [page, setPage] = useState<Page>("shield");
  const [theme, setTheme] = useState<Theme>("dark");

  const { toasts, pushToast, dismissToast } = useToast();
  const subs = useSubscriptions(pushToast);
  const manual = useManualServers(pushToast);
  const settings = useSettings();
  // все узлы (подписки + добавленные вручную) — для трея и выбора из него; useMemo,
  // чтобы sync_tray (эффект в useConnection) не дёргался на каждый рендер
  const allServers = useMemo(
    () => [...subs.subscriptions.flatMap((subscription) => subscription.servers), ...manual.servers],
    [subs.subscriptions, manual.servers],
  );
  const connection = useConnection({
    servers: allServers,
    ruBypass: settings.ruBypass,
    killSwitch: settings.killSwitch,
    pushToast,
  });
  const { traffic, memoryBytes, memoryDebug } = useTrafficStats(connection.status.connected, pushToast);
  const update = useAppUpdate(pushToast);
  const geo = useGeoUpdates(pushToast);
  useAppBootstrap({ settings, subs, manual, setSelectedServer: connection.setSelectedServer });

  const showTabs = page !== "nodes";

  return (
    <div className={`window vrox-${theme}`}>
      <ToastStack toasts={toasts} onDismiss={dismissToast} />

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
          manual={manual}
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
