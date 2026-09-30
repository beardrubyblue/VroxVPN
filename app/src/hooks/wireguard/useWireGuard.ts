import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { PingResult, Server } from "@/types";

type TPushToast = (text: string, kind?: "error" | "info") => void;

// Импортированные WireGuard/AmneziaWG-конфиги (Rust wireguard/). Каждый
// конфиг — отдельный узел; группа WireGuard на экране Nodes.
export function useWireGuard(pushToast: TPushToast) {
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
      const list = await invoke<Server[]>("list_wireguard");
      setServers(list);
      ping(list);
      return list;
    } catch {
      return [];
    }
  }

  async function importConf(name: string, conf: string): Promise<boolean> {
    try {
      const server = await invoke<Server>("import_wireguard", { name, conf });
      setServers((prev) => [...prev, server]);
      ping([server]);
      pushToast(`WireGuard ${server.name} added`);
      return true;
    } catch (err) {
      pushToast(String(err), "error");
      return false;
    }
  }

  async function rename(id: string, name: string) {
    try {
      await invoke("rename_wireguard", { id, name });
      setServers((prev) => prev.map((server) => (server.wireguard?.id === id ? { ...server, name } : server)));
    } catch (err) {
      pushToast(String(err), "error");
    }
  }

  async function remove(id: string) {
    try {
      await invoke("delete_wireguard", { id });
      setServers((prev) => prev.filter((server) => server.wireguard?.id !== id));
    } catch (err) {
      pushToast(String(err), "error");
    }
  }

  return { servers, pings, load, importConf, rename, remove };
}
