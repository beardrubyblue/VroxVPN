import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { subscriptionNameFromUrl } from "@/utils/format";
import type { CachedSubscription, PingResult, Server, Subscription, SubscriptionMeta } from "@/types";
import {
  cachedSubscription,
  emptySubscription,
  fetchSubscription,
  loadCache,
  persistMetas,
  toMetas,
} from "./subscription-store";

type TPushToast = (text: string, kind?: "error" | "info", detail?: string) => void;

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
  // вычищать рабочие серверы (группа подсвечивается красным). null —
  // загрузка не удалась.
  async function refresh(url: string): Promise<Server[] | null> {
    patch(url, { refreshing: true });
    try {
      const fetched = await fetchSubscription(url);
      patch(url, { ...fetched, refreshing: false, error: "" });
      ping(url, fetched.servers);
      return fetched.servers;
    } catch (err) {
      patch(url, { refreshing: false, error: String(err) });
      return null;
    }
  }

  async function refreshAll() {
    await Promise.all(subscriptionsRef.current.map((sub) => refresh(sub.url)));
  }

  // Возвращает серверы всех подписок — useAppBootstrap ищет среди них
  // последний выбранный.
  // Раньше узлы подписок не сохранялись вовсе (в settings.json только
  // url и имя) — после запуска экран Nodes был пуст до конца загрузки, а
  // без сети пуст совсем. Теперь сначала показываем кеш и не ждём
  // загрузку подписок, у которых он есть; ждём только подписки без кеша.
  async function loadFromMetas(metas: SubscriptionMeta[]): Promise<Server[]> {
    const cache = await loadCache();
    setSubscriptions(
      metas.map((meta) => (cache[meta.url] ? cachedSubscription(meta, cache[meta.url]) : emptySubscription(meta))),
    );
    const loaded = await Promise.all(metas.map((meta) => loadOne(meta.url, cache[meta.url])));
    return loaded.flat();
  }

  async function loadOne(url: string, cached: CachedSubscription | undefined): Promise<Server[]> {
    if (!cached) return (await refresh(url)) ?? [];
    // загрузка не удалась — пингуем хотя бы кешированные узлы
    refresh(url).then((fetched) => {
      if (!fetched) ping(url, cached.servers);
    });
    return cached.servers;
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
      pushToast("Subscription added", "info", `${added.name} · ${added.servers.length} servers`);
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
    invoke("forget_subscription", { url }).catch(() => {});
    await persistMetas(toMetas(subscriptionsRef.current).filter((meta) => meta.url !== url));
  }

  return { subscriptions, loadFromMetas, addFromUrl, refresh, refreshAll, rename, remove };
}
