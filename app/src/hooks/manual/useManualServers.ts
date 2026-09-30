import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { PingResult, Server } from "@/types";

type TPushToast = (text: string, kind?: "error" | "info", detail?: string) => void;

// Узлы, добавленные вручную (Rust manual/): ссылка hysteria2:// или .conf
// WireGuard/AmneziaWG. Группа «Added manually» на экране Nodes — источник,
// протокол показан у каждого узла.
export function useManualServers(pushToast: TPushToast) {
  const [servers, setServers] = useState<Server[]>([]);
  const [pings, setPings] = useState<Record<string, PingResult>>({});

  async function ping(list: Server[]) {
    if (list.length === 0) return;
    try {
      const results = await invoke<PingResult[]>("ping_servers", { servers: list });
      setPings((prev) => ({ ...prev, ...Object.fromEntries(results.map((result) => [result.name, result])) }));
    } catch {
      // пинг необязателен — без него просто прочерк
    }
  }

  async function load(): Promise<Server[]> {
    try {
      const list = await invoke<Server[]>("list_manual_servers");
      setServers(list);
      ping(list);
      return list;
    } catch {
      return [];
    }
  }

  // text — ссылка hysteria2:// или .conf; протокол определяет Rust
  async function importText(name: string, text: string): Promise<boolean> {
    try {
      const server = await invoke<Server>("import_manual_server", { name, text });
      setServers((prev) => [...prev, server]);
      ping([server]);
      pushToast("Server added", "info", `${server.name} · ${server.host}:${server.port}`);
      return true;
    } catch (err) {
      pushToast(String(err), "error");
      return false;
    }
  }

  async function rename(id: string, name: string) {
    try {
      await invoke("rename_manual_server", { id, name });
      setServers((prev) => prev.map((server) => (server.manual_id === id ? { ...server, name } : server)));
    } catch (err) {
      pushToast(String(err), "error");
    }
  }

  async function remove(id: string) {
    try {
      await invoke("delete_manual_server", { id });
      setServers((prev) => prev.filter((server) => server.manual_id !== id));
    } catch (err) {
      pushToast(String(err), "error");
    }
  }

  return { servers, pings, load, importText, rename, remove };
}
