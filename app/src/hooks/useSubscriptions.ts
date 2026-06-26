import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { subscriptionNameFromUrl } from "@/utils/format";
import type { PingResult, Server, Subscription, SubscriptionMeta } from "@/types";

async function fetchAndStoreServers(meta: SubscriptionMeta): Promise<Subscription> {
  try {
    const servers = await invoke<Server[]>("fetch_servers", { url: meta.url });
    return { ...meta, servers, pings: {}, pinging: false, refreshing: false, error: "" };
  } catch (err) {
    return { ...meta, servers: [], pings: {}, pinging: false, refreshing: false, error: String(err) };
  }
}

async function persistSubscriptionMetas(subs: Subscription[]) {
  const metas: SubscriptionMeta[] = subs.map((s) => ({ url: s.url, name: s.name }));
  await invoke("set_setting", { key: "subscriptions", value: metas });
}

export function useSubscriptions(pushToast: (text: string, kind?: "error" | "info") => void) {
  const [subscriptions, setSubscriptions] = useState<Subscription[]>([]);
  // тред-актуальная копия для слушателей событий трея (см.
  // App.tsx::tray-select-server) — subscriptions меняется часто (на
  // каждый пинг/обновление), а переподписываться на каждое изменение
  // не нужно
  const subscriptionsRef = useRef<Subscription[]>([]);
  useEffect(() => {
    subscriptionsRef.current = subscriptions;
  }, [subscriptions]);

  async function loadFromMetas(metas: SubscriptionMeta[]): Promise<Subscription[]> {
    const loaded = await Promise.all(metas.map(fetchAndStoreServers));
    setSubscriptions(loaded);
    return loaded;
  }

  async function addFromUrl(url: string): Promise<boolean> {
    if (subscriptions.some((s) => s.url === url)) {
      pushToast("такая подписка уже добавлена", "error");
      return false;
    }
    const meta: SubscriptionMeta = { url, name: subscriptionNameFromUrl(url) };
    const loaded = await fetchAndStoreServers(meta);
    if (loaded.error) {
      pushToast(loaded.error, "error");
      return false;
    }
    const next = [...subscriptions, loaded];
    setSubscriptions(next);
    await persistSubscriptionMetas(next);
    pushToast(`Подписка ${loaded.name} добавлена — ${loaded.servers.length} серверов`);
    return true;
  }

  async function refresh(url: string) {
    setSubscriptions((prev) => prev.map((s) => (s.url === url ? { ...s, refreshing: true } : s)));
    const current = subscriptions.find((s) => s.url === url);
    if (!current) return;
    const loaded = await fetchAndStoreServers({ url: current.url, name: current.name });
    if (loaded.error) {
      pushToast(loaded.error, "error");
      setSubscriptions((prev) => prev.map((s) => (s.url === url ? { ...s, refreshing: false } : s)));
      return;
    }
    setSubscriptions((prev) => prev.map((s) => (s.url === url ? loaded : s)));
  }

  async function remove(url: string): Promise<Subscription[]> {
    const next = subscriptions.filter((s) => s.url !== url);
    setSubscriptions(next);
    await persistSubscriptionMetas(next);
    return next;
  }

  async function ping(url: string) {
    setSubscriptions((prev) => prev.map((s) => (s.url === url ? { ...s, pinging: true } : s)));
    const sub = subscriptions.find((s) => s.url === url);
    if (!sub) return;
    const results = await invoke<PingResult[]>("ping_servers", { servers: sub.servers });
    const pings: Subscription["pings"] = {};
    for (const r of results) pings[r.name] = r;
    setSubscriptions((prev) => prev.map((s) => (s.url === url ? { ...s, pings, pinging: false } : s)));
  }

  return { subscriptions, subscriptionsRef, loadFromMetas, addFromUrl, refresh, remove, ping };
}
