export interface ConnectionStatus {
  connected: boolean;
  server_name: string | null;
}

export interface TrafficTotals {
  upload_bytes: number;
  download_bytes: number;
  memory_bytes: number;
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

export interface Subscription extends SubscriptionMeta {
  servers: Server[];
  pings: Record<string, PingResult>;
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
