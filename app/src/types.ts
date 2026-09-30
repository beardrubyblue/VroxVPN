export interface ConnectionStatus {
  connected: boolean;
  server_name: string | null;
}

// Детальная разбивка памяти тоннеля — только macOS/iOS (см.
// engine::MemoryDebug в Rust). Поля зеркалят netunnel.go::GetStats.
export interface MemoryDebug {
  heap_in_use: number;
  heap_sys: number;
  goroutines: number;
  tcp_relays: number;
  udp_relays: number;
  registry_size: number;
  avail_mem: number;
}

export interface TrafficTotals {
  upload_bytes: number;
  download_bytes: number;
  memory_bytes: number;
  debug?: MemoryDebug;
}

export interface TrafficDisplay {
  upSpeed: number;
  downSpeed: number;
  totalUp: number;
  totalDown: number;
}

export interface Server {
  name: string;
  host: string;
  port: number;
  password: string;
  sni: string;
  insecure: boolean;
  obfs: string;
  obfs_password: string;
  pin_sha256: string;
  quic: Record<string, unknown>;
  raw_uri: string;
  // Есть — WireGuard/AmneziaWG-узел из импортированного .conf (Rust
  // wireguard/), нет — hysteria2 из подписки. UI нужен только id.
  wireguard?: { id: string };
}

export interface PingResult {
  name: string;
  latency_ms: number | null;
  error: string | null;
}

export interface SubscriptionMeta {
  url: string;
  name: string;
}

// Subscription-Userinfo от сервера подписки (байты и unix-время в
// секундах); 0 — поле не прислано.
export interface SubscriptionUsage {
  upload: number;
  download: number;
  total: number;
  expire: number;
}

// Ответ Rust-команды fetch_subscription (subscription.rs::SubscriptionData).
export interface SubscriptionData {
  servers: Server[];
  userinfo: { fields: Record<string, number> };
}

export interface Subscription extends SubscriptionMeta {
  servers: Server[];
  pings: Record<string, PingResult>;
  usage: SubscriptionUsage | null;
  updatedAt: number | null;
  pinging: boolean;
  refreshing: boolean;
  error: string;
}

export interface Settings {
  subscriptions?: SubscriptionMeta[];
  last_selected_server: string;
  ru_bypass_enabled: boolean;
  kill_switch_enabled: boolean;
}

// День истории трафика (Rust traffic_history::DayTraffic, файл пишет
// расширение на iOS / Rust на Linux). date — локальная YYYY-MM-DD.
export interface DayTraffic {
  date: string;
  upload: number;
  download: number;
}

export interface UpdateCheck {
  current: string;
  latest: string;
  update_available: boolean;
  download_url: string;
  changelog: string;
  sha256: string;
  auto_installable: boolean;
}

export interface UpdateInfo {
  version: string;
  notes: string;
  downloadUrl: string;
  sha256: string;
  autoInstallable: boolean;
}

export interface Toast {
  text: string;
  kind: "error" | "info";
}
