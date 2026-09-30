import type { Server } from "@/types";

// Название протокола узла для строки под именем на экране Nodes (вместо
// домена/IP сервера): WireGuard с параметрами маскировки — AmneziaWG.
export function protocolLabel(server: Server): string {
  if (!server.wireguard) return "Hysteria2";
  const hasObfuscation = Object.keys(server.wireguard.obfuscation ?? {}).length > 0;
  return hasObfuscation ? "AmneziaWG" : "WireGuard";
}
