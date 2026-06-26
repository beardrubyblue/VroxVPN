import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { readText } from "@tauri-apps/plugin-clipboard-manager";
import { AddSubscriptionSheet } from "@/components/AddSubscriptionSheet";
import { BottomAction } from "@/components/BottomAction";
import { DeleteConfirmSheet } from "@/components/DeleteConfirmSheet";
import { MemoryCard } from "@/components/MemoryCard";
import { SettingsPage } from "@/components/SettingsPage";
import { SubscriptionList } from "@/components/subscriptions";
import { ToastBanner } from "@/components/ToastBanner";
import { TrafficCard } from "@/components/TrafficCard";
import { ViewSwitcher } from "@/components/ViewSwitcher";
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
import "./App.css";

function App() {
  const [page, setPage] = useState<"home" | "settings">("home");

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
  const { traffic, memoryBytes } = useTrafficStats(connection.status.connected, pushToast);
  const update = useAppUpdate(pushToast);
  const geo = useGeoUpdates(pushToast);
  const addSheet = useSheet();
  const deleteSheet = useSheet();

  const [newUrl, setNewUrl] = useState("");
  const [addError, setAddError] = useState("");
  const [confirmTarget, setConfirmTarget] = useState<{ url: string; name: string } | null>(null);

  // подгрузка сохранённых настроек/подписок при старте — тот же
  // settings.json, что у старого Python-приложения (core/settings.py)
  useAppBootstrap({ settings, subs, setSelectedServer: connection.setSelectedServer });

  function openAddSheet() {
    setNewUrl("");
    setAddError("");
    addSheet.show();
  }

  async function confirmAddSubscription() {
    if (!newUrl.trim()) {
      setAddError("введи URL подписки");
      return;
    }
    setAddError("");
    if (await subs.addFromUrl(newUrl.trim())) {
      addSheet.hide();
    }
  }

  async function pasteFromClipboard() {
    let text: string | null;
    try {
      text = await readText();
    } catch {
      pushToast("нет доступа к буферу обмена", "error");
      return;
    }
    if (!text || !text.trim()) {
      pushToast("буфер обмена пуст", "error");
      return;
    }
    await subs.addFromUrl(text.trim());
  }

  function openDeleteConfirm(url: string, name: string) {
    setConfirmTarget({ url, name });
    deleteSheet.show();
  }

  async function confirmDeleteSubscription() {
    if (confirmTarget) {
      const next = await subs.remove(confirmTarget.url);
      const selected = connection.selectedServer;
      if (selected && !next.some((s) => s.servers.some((srv) => srv.name === selected.name))) {
        connection.setSelectedServer(null);
      }
    }
    deleteSheet.hide();
  }

  return (
    <div className="window">
      <ToastBanner toast={toast} />

      {page === "home" ? (
        <main className="page">
          <MemoryCard memoryBytes={connection.status.connected ? memoryBytes : 0} />
          {connection.status.connected && traffic && <TrafficCard traffic={traffic} />}
          <SubscriptionList
            subscriptions={subs.subscriptions}
            selectedServerName={connection.selectedServer?.name}
            selectable={!connection.status.connected}
            onRefresh={subs.refresh}
            onPing={subs.ping}
            onDeleteRequest={openDeleteConfirm}
            onSelectServer={connection.setSelectedServer}
            onPingError={(error) => pushToast(error, "error")}
          />
        </main>
      ) : (
        <SettingsPage
          ruBypass={settings.ruBypass}
          onRuBypassChange={settings.onRuBypassChange}
          connected={connection.status.connected}
          geoipLoading={geo.geoipLoading}
          onUpdateGeoip={geo.updateGeoip}
          geositeLoading={geo.geositeLoading}
          onUpdateGeosite={geo.updateGeosite}
          bypassStatus={geo.bypassStatus}
          killSwitch={settings.killSwitch}
          onKillSwitchChange={settings.onKillSwitchChange}
          updateChecking={update.checking}
          onCheckUpdate={update.check}
          updateInfo={update.info}
          updateInstalling={update.installing}
          onInstallUpdate={update.install}
          onQuit={() => invoke("quit_app")}
        />
      )}

      {page === "home" && (
        <BottomAction
          hasSubscriptions={subs.subscriptions.length > 0}
          onAdd={openAddSheet}
          onPaste={pasteFromClipboard}
          connected={connection.status.connected}
          busy={connection.busy}
          serverName={connection.status.server_name}
          selectedServerName={connection.selectedServer?.name}
          onToggle={connection.toggleConnection}
        />
      )}

      <ViewSwitcher page={page} onChange={setPage} />

      <AddSubscriptionSheet
        open={addSheet.open}
        visible={addSheet.visible}
        url={newUrl}
        onUrlChange={setNewUrl}
        error={addError}
        onConfirm={confirmAddSubscription}
        onClose={addSheet.hide}
      />

      <DeleteConfirmSheet
        open={deleteSheet.open}
        visible={deleteSheet.visible}
        targetName={confirmTarget?.name}
        onCancel={deleteSheet.hide}
        onConfirm={confirmDeleteSubscription}
      />
    </div>
  );
}

export default App;
