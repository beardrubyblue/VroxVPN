import type { PingResult, Server } from "@/types";

// All / Fastest — все узлы (Fastest — по пингу); Hysteria2 / WireGuard —
// фильтр по протоколу (WireGuard включает AmneziaWG). Группы — источники,
// поэтому протокол фильтруется внутри каждой группы.
export const NodesTabEnum = {
  All: "all",
  Fastest: "fast",
  Hysteria2: "hysteria2",
  WireGuard: "wireguard",
} as const;

export type NodesTabEnum = (typeof NodesTabEnum)[keyof typeof NodesTabEnum];

// Фильтр по поиску и протоколу + сортировка по пингу для Fastest —
// применяется внутри каждой группы отдельно.
export function filterNodes(
  servers: Server[],
  pings: Record<string, PingResult>,
  query: string,
  tab: NodesTabEnum,
): Server[] {
  const needle = query.trim().toLowerCase();
  const matched = servers.filter((server) => {
    if (needle && !server.name.toLowerCase().includes(needle)) return false;
    if (tab === NodesTabEnum.Hysteria2) return !server.wireguard;
    if (tab === NodesTabEnum.WireGuard) return Boolean(server.wireguard);
    return true;
  });
  if (tab !== NodesTabEnum.Fastest) return matched;
  const latency = (server: Server) => pings[server.name]?.latency_ms ?? Infinity;
  return [...matched].sort((first, second) => latency(first) - latency(second));
}
