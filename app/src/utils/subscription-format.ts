import { formatBytes } from "@/utils/format";
import type { Subscription, SubscriptionUsage } from "@/types";

const MINUTE_MS = 60 * 1000;
const HOUR_MS = 60 * MINUTE_MS;
const DAY_MS = 24 * HOUR_MS;

// Subscription-Userinfo → usage; null, если сервер подписки ничего
// полезного не прислал (тогда в заголовке группы нет строки трафика).
export function usageFromUserInfo(fields: Record<string, number>): SubscriptionUsage | null {
  const usage = {
    upload: fields.upload ?? 0,
    download: fields.download ?? 0,
    total: fields.total ?? 0,
    expire: fields.expire ?? 0,
  };
  return usage.total > 0 || usage.expire > 0 ? usage : null;
}

// Доля израсходованного трафика 0..100 или null без лимита.
export function usagePercent(usage: SubscriptionUsage | null): number | null {
  if (!usage || usage.total <= 0) return null;
  return Math.min(100, Math.round(((usage.upload + usage.download) / usage.total) * 100));
}

function formatAgo(timestamp: number): string {
  const elapsed = Date.now() - timestamp;
  if (elapsed < MINUTE_MS) return "JUST NOW";
  if (elapsed < HOUR_MS) return `${Math.floor(elapsed / MINUTE_MS)}M AGO`;
  if (elapsed < DAY_MS) return `${Math.floor(elapsed / HOUR_MS)}H AGO`;
  return `${Math.floor(elapsed / DAY_MS)}D AGO`;
}

function formatExpire(expireSeconds: number): string {
  const date = new Date(expireSeconds * 1000);
  const day = String(date.getDate()).padStart(2, "0");
  const month = String(date.getMonth() + 1).padStart(2, "0");
  return `TILL ${day}.${month}`;
}

// Строка-подпись под названием подписки: «3 NODES · 38 GB / 100 GB · TILL 12.11».
export function formatSubscriptionMeta(subscription: Subscription): string {
  const { servers, usage, updatedAt, refreshing, error } = subscription;
  if (refreshing && servers.length === 0) return "LOADING…";
  if (error) return servers.length > 0 ? "UPDATE FAILED · SHOWING LAST LIST" : "UPDATE FAILED";

  const parts = [`${servers.length} NODES`];
  if (usage && usage.total > 0) {
    parts.push(`${formatBytes(usage.upload + usage.download)} / ${formatBytes(usage.total)}`);
  }
  if (usage && usage.expire > 0) parts.push(formatExpire(usage.expire));
  if (!usage && updatedAt) parts.push(`UPDATED ${formatAgo(updatedAt)}`);
  return parts.join(" · ");
}
