import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { Server, Settings } from "@/types";
import type { useSettings } from "./useSettings";
import type { useSubscriptions } from "./useSubscriptions";

interface UseAppBootstrapArgs {
  settings: ReturnType<typeof useSettings>;
  subs: ReturnType<typeof useSubscriptions>;
  setSelectedServer: (server: Server | null) => void;
}

// Подгрузка сохранённых настроек/подписок при старте — тот же
// settings.json, что у старого Python-приложения (core/settings.py в
// ветке main).
export function useAppBootstrap({ settings, subs, setSelectedServer }: UseAppBootstrapArgs) {
  useEffect(() => {
    (async () => {
      const saved = await invoke<Settings>("get_settings");
      settings.setRuBypass(saved.ru_bypass_enabled);
      settings.setKillSwitch(saved.kill_switch_enabled);
      settings.setIdleTimeoutSeconds(saved.idle_timeout_seconds);
      settings.setMaxTcpConnections(saved.max_tcp_connections);
      settings.setMaxUdpConnections(saved.max_udp_connections);
      const loaded = await subs.loadFromMetas(saved.subscriptions ?? []);
      if (saved.last_selected_server) {
        for (const sub of loaded) {
          const found = sub.servers.find((s) => s.name === saved.last_selected_server);
          if (found) {
            setSelectedServer(found);
            break;
          }
        }
      }
    })();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
}
