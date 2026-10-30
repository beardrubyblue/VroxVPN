import { invoke } from "@tauri-apps/api/core";
import { usageFromUserInfo } from "@/utils/subscription-format";
import type { CachedSubscription, Subscription, SubscriptionData, SubscriptionMeta } from "@/types";

// Обёртки над invoke-командами подписок и сборка состояния группы —
// без React, чтобы useSubscriptions остался про состояние.

export function emptySubscription(meta: SubscriptionMeta): Subscription {
  return { ...meta, servers: [], pings: {}, usage: null, updatedAt: null, pinging: false, refreshing: true, error: "" };
}

// Узлы из кеша (subscription_cache.rs) — группа видна сразу, а
// refreshing остаётся true, пока фоновая загрузка их не подменит.
export function cachedSubscription(meta: SubscriptionMeta, cached: CachedSubscription): Subscription {
  return {
    ...emptySubscription(meta),
    servers: cached.servers,
    usage: usageFromUserInfo(cached.userinfo.fields),
    updatedAt: cached.updated_at,
  };
}

// Кеш не прочитался — не повод ломать запуск: узлы придут с загрузкой.
export async function loadCache(): Promise<Record<string, CachedSubscription>> {
  try {
    return await invoke<Record<string, CachedSubscription>>("get_cached_subscriptions");
  } catch {
    return {};
  }
}

export async function fetchSubscription(url: string) {
  const data = await invoke<SubscriptionData>("fetch_subscription", { url });
  return { servers: data.servers, usage: usageFromUserInfo(data.userinfo.fields), updatedAt: Date.now() };
}

export async function persistMetas(metas: SubscriptionMeta[]) {
  await invoke("set_setting", { key: "subscriptions", value: metas });
}

export function toMetas(subscriptions: Subscription[]): SubscriptionMeta[] {
  return subscriptions.map(({ url, name }) => ({ url, name }));
}
