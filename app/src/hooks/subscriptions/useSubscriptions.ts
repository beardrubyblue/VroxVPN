import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { subscriptionNameFromUrl } from "@/utils/format";
import { usageFromUserInfo } from "@/utils/subscription-format";
import type { PingResult, Server, Subscription, SubscriptionData, SubscriptionMeta } from "@/types";

type TPushToast = (text: string, kind?: "error" | "info") => void;

function emptySubscription(meta: SubscriptionMeta): Subscription {
  return { ...meta, servers: [], pings: {}, usage: null, updatedAt: null, pinging: false, refreshing: true, error: "" };
}

async function fetchSubscription(url: string) {
  const data = await invoke<SubscriptionData>("fetch_subscription", { url });
  return { servers: data.servers, usage: usageFromUserInfo(data.userinfo.fields), updatedAt: Date.now() };
}

async function persistMetas(metas: SubscriptionMeta[]) {
  await invoke("set_setting", { key: "subscriptions", value: metas });
}

function toMetas(subscriptions: Subscription[]): SubscriptionMeta[] {
  return subscriptions.map(({ url, name }) => ({ url, name }));
}

export function useSubscriptions(pushToast: TPushToast) {
  const [subscriptions, setSubscriptions] = useState<Subscription[]>([]);
  // актуальная копия для вычисления списка метаданных при сохранении и
  // проверки дублей — subscriptions меняется на каждый пинг/обновление
  const subscriptionsRef = useRef<Subscription[]>([]);
  useEffect(() => {
    subscriptionsRef.current = subscriptions;
  }, [subscriptions]);

  function patch(url: string, changes: Partial<Subscription>) {
    setSubscriptions((prev) => prev.map((sub) => (sub.url === url ? { ...sub, ...changes } : sub)));
  }

  // После редизайна (bf782fe) пинг никто не вызывал — у всех серверов
  // стоял прочерк, сортировка Fastest ничего не делала. Теперь пингуем
  // после каждой загрузки подписки.
  async function ping(url: string, servers: Server[]) {
    patch(url, { pinging: true });
    try {
      const results = await invoke<PingResult[]>("ping_servers", { servers });
      patch(url, { pings: Object.fromEntries(results.map((result) => [result.name, result])), pinging: false });
    } catch {
      patch(url, { pinging: false });
    }
  }

  // Загрузка/обновление одной подписки. При ошибке прежний список
  // серверов остаётся — сервер подписки, недоступный минуту, не должен
  // вычищать рабочие серверы (группа подсвечивается красным).
  async function refresh(url: string): Promise<Server[]> {
    patch(url, { refreshing: true });
    try {
      const fetched = await fetchSubscription(url);
      patch(url, { ...fetched, refreshing: false, error: "" });
      ping(url, fetched.servers);
      return fetched.servers;
    } catch (err) {
      patch(url, { refreshing: false, error: String(err) });
      return [];
    }
  }

  async function refreshAll() {
    await Promise.all(subscriptionsRef.current.map((sub) => refresh(sub.url)));
  }

  // Возвращает серверы всех подписок — useAppBootstrap ищет среди них
  // последний выбранный.
  async function loadFromMetas(metas: SubscriptionMeta[]): Promise<Server[]> {
    setSubscriptions(metas.map(emptySubscription));
    const loaded = await Promise.all(metas.map((meta) => refresh(meta.url)));
    return loaded.flat();
  }

  async function addFromUrl(url: string): Promise<boolean> {
    if (subscriptionsRef.current.some((sub) => sub.url === url)) {
      pushToast("This subscription is already added", "error");
      return false;
    }
    try {
      const fetched = await fetchSubscription(url);
      const added: Subscription = {
        ...emptySubscription({ url, name: subscriptionNameFromUrl(url) }),
        ...fetched,
        refreshing: false,
      };
      setSubscriptions((prev) => [...prev, added]);
      await persistMetas([...toMetas(subscriptionsRef.current), { url, name: added.name }]);
      pushToast(`Subscription ${added.name} added — ${added.servers.length} servers`);
      ping(url, added.servers);
      return true;
    } catch (err) {
      pushToast(String(err), "error");
      return false;
    }
  }

  async function rename(url: string, name: string) {
    patch(url, { name });
    await persistMetas(toMetas(subscriptionsRef.current).map((meta) => (meta.url === url ? { ...meta, name } : meta)));
  }

  async function remove(url: string) {
    setSubscriptions((prev) => prev.filter((sub) => sub.url !== url));
    await persistMetas(toMetas(subscriptionsRef.current).filter((meta) => meta.url !== url));
  }

  return { subscriptions, loadFromMetas, addFromUrl, refresh, refreshAll, rename, remove };
}
