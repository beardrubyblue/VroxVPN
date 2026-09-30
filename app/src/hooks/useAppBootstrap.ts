import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { Server, Settings } from "@/types";
import type { useSettings } from "./useSettings";
import type { useSubscriptions } from "./subscriptions";
import type { useWireGuard } from "./wireguard";

interface UseAppBootstrapArgs {
  settings: ReturnType<typeof useSettings>;
  subs: ReturnType<typeof useSubscriptions>;
  wireguard: ReturnType<typeof useWireGuard>;
  setSelectedServer: (server: Server | null) => void;
}

// Подгрузка сохранённых настроек/подписок при старте — тот же
// settings.json, что у старого Python-приложения (core/settings.py в
// ветке main).
export function useAppBootstrap({ settings, subs, wireguard, setSelectedServer }: UseAppBootstrapArgs) {
  useEffect(() => {
    (async () => {
      const saved = await invoke<Settings>("get_settings");
      settings.setRuBypass(saved.ru_bypass_enabled);
      settings.setKillSwitch(saved.kill_switch_enabled);
      const [subscriptionServers, wireguardServers] = await Promise.all([
        subs.loadFromMetas(saved.subscriptions ?? []),
        wireguard.load(),
      ]);
      const servers = [...subscriptionServers, ...wireguardServers];
      const lastSelected = servers.find((server) => server.name === saved.last_selected_server);
      if (lastSelected) setSelectedServer(lastSelected);
    })();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
}
