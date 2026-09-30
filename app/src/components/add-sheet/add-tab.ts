// Вкладки шторки «+»: источник сервера — подписка или один сервер
// (ссылка hysteria2:// или .conf WireGuard), а не протокол.
export const AddTabEnum = {
  Subscription: "subscription",
  Server: "server",
} as const;

export type AddTabEnum = (typeof AddTabEnum)[keyof typeof AddTabEnum];
