import type { PingResult, Server } from "@/types";

export const NodesTabEnum = {
  All: "all",
  Fastest: "fast",
} as const;

export type NodesTabEnum = (typeof NodesTabEnum)[keyof typeof NodesTabEnum];

// Фильтр по поиску + сортировка по пингу для вкладки Fastest — применяется
// внутри каждой группы подписки отдельно.
export function filterNodes(
  servers: Server[],
  pings: Record<string, PingResult>,
  query: string,
  tab: NodesTabEnum,
): Server[] {
  const needle = query.trim().toLowerCase();
  const matched = needle ? servers.filter((server) => server.name.toLowerCase().includes(needle)) : servers;
  if (tab !== NodesTabEnum.Fastest) return matched;
  const latency = (server: Server) => pings[server.name]?.latency_ms ?? Infinity;
  return [...matched].sort((first, second) => latency(first) - latency(second));
}
