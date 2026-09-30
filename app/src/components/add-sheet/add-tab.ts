// Вкладки шторки «+»: подписка hysteria2 или конфиг WireGuard/AmneziaWG.
export const AddTabEnum = {
  Subscription: "subscription",
  WireGuard: "wireguard",
} as const;

export type AddTabEnum = (typeof AddTabEnum)[keyof typeof AddTabEnum];
