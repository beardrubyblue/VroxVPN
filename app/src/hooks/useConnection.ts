import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { ConnectionStatus, Server, Subscription } from "@/types";

interface UseConnectionArgs {
  subscriptions: Subscription[];
  subscriptionsRef: { current: Subscription[] };
  ruBypass: boolean;
  killSwitch: boolean;
  pushToast: (text: string, kind?: "error" | "info") => void;
}

export function useConnection({ subscriptions, subscriptionsRef, ruBypass, killSwitch, pushToast }: UseConnectionArgs) {
  const [status, setStatus] = useState<ConnectionStatus>({ connected: false, server_name: null });
  const [selectedServer, setSelectedServer] = useState<Server | null>(null);
  const [busy, setBusy] = useState(false);

  async function refreshStatus() {
    setStatus(await invoke<ConnectionStatus>("get_status"));
  }

  async function toggleConnection() {
    if (busy) return;
    setBusy(true);
    try {
      if (status.connected) {
        await invoke("disconnect");
      } else {
        if (!selectedServer) {
          setBusy(false);
          return;
        }
        await invoke("connect", { server: selectedServer, ruBypass, killSwitch });
        await invoke("set_setting", { key: "last_selected_server", value: selectedServer.name });
      }
    } catch (err) {
      pushToast(String(err), "error");
    }
    setBusy(false);
    await refreshStatus();
  }

  // движок может разорвать соединение сам, не по нашей команде disconnect
  // (например, сервер уронил QUIC-сессию) — без этого слушателя UI
  // продолжал бы показывать "подключено" при мёртвом тоннеле, пока
  // пользователь не откроет приложение заново
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    (async () => {
      unlisten = await listen("vpn-disconnected-unexpectedly", () => {
        pushToast("Соединение разорвано", "error");
        refreshStatus();
      });
    })();
    return () => unlisten?.();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // подписку на "выбрать сервер"/"переключить подключение" из трея
  // держим через subscriptionsRef (она меняется часто — на каждый
  // пинг/обновление), а на toggle и ruBypass переподписываемся, это
  // происходит редко
  useEffect(() => {
    let unlistenToggle: (() => void) | undefined;
    let unlistenSelect: (() => void) | undefined;
    (async () => {
      unlistenToggle = await listen("tray-toggle-connection", () => {
        toggleConnection();
      });
      unlistenSelect = await listen<string>("tray-select-server", (event) => {
        const name = event.payload;
        for (const sub of subscriptionsRef.current) {
          const found = sub.servers.find((s) => s.name === name);
          if (found) {
            setSelectedServer(found);
            break;
          }
        }
      });
    })();
    return () => {
      unlistenToggle?.();
      unlistenSelect?.();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [status.connected, selectedServer, ruBypass]);

  // зеркалим текущее состояние в меню трея
  useEffect(() => {
    const servers = subscriptions.flatMap((s) => s.servers.map((srv) => srv.name));
    invoke("sync_tray", {
      connected: status.connected,
      currentServer: status.connected ? status.server_name : selectedServer?.name ?? null,
      servers,
    });
  }, [status, selectedServer, subscriptions]);

  return { status, selectedServer, setSelectedServer, busy, toggleConnection };
}
