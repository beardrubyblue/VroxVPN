import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { readText } from "@tauri-apps/plugin-clipboard-manager";
import "./App.css";

interface ConnectionStatus {
  connected: boolean;
  server_name: string | null;
}

interface Server {
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

interface PingResult {
  name: string;
  latency_ms: number | null;
}

interface SubscriptionMeta {
  url: string;
  name: string;
}

interface Subscription extends SubscriptionMeta {
  servers: Server[];
  pings: Record<string, number | null>;
  pinging: boolean;
  refreshing: boolean;
  error: string;
}

interface Settings {
  subscriptions?: SubscriptionMeta[];
  last_selected_server: string;
  ru_bypass_enabled: boolean;
  kill_switch_enabled: boolean;
}

interface UpdateCheck {
  current: string;
  latest: string;
  update_available: boolean;
  download_url: string;
  changelog: string;
  sha256: string;
}

interface Toast {
  text: string;
  kind: "error" | "info";
}

function subscriptionNameFromUrl(url: string): string {
  try {
    return new URL(url).hostname;
  } catch {
    return url;
  }
}

function RefreshIcon() {
  return (
    <svg className="icon-svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M21 12a9 9 0 0 0-9-9 9.75 9.75 0 0 0-6.74 2.74L3 8" />
      <path d="M3 3v5h5" />
      <path d="M3 12a9 9 0 0 0 9 9 9.75 9.75 0 0 0 6.74-2.74L21 16" />
      <path d="M16 16h5v5" />
    </svg>
  );
}

function PingIcon() {
  return (
    <svg className="icon-svg" viewBox="0 0 24 22" fill="currentColor">
      <rect x="1" y="14" width="4" height="7" rx="1" />
      <rect x="8" y="9" width="4" height="12" rx="1" />
      <rect x="15" y="5" width="4" height="16" rx="1" />
    </svg>
  );
}

function TrashIcon() {
  return (
    <svg className="icon-svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M3 6h18" />
      <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6" />
      <path d="M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
      <line x1="10" y1="11" x2="10" y2="17" />
      <line x1="14" y1="11" x2="14" y2="17" />
    </svg>
  );
}

function Switch({
  checked,
  onChange,
  disabled,
}: {
  checked: boolean;
  onChange: (v: boolean) => void;
  disabled?: boolean;
}) {
  return (
    <label className="switch">
      <input
        type="checkbox"
        checked={checked}
        disabled={disabled}
        onChange={(e) => onChange(e.currentTarget.checked)}
      />
      <span className="slider" />
    </label>
  );
}

function App() {
  const [page, setPage] = useState<"home" | "settings">("home");
  const [status, setStatus] = useState<ConnectionStatus>({
    connected: false,
    server_name: null,
  });
  const [toast, setToast] = useState<Toast | null>(null);

  const [subscriptions, setSubscriptions] = useState<Subscription[]>([]);
  const subscriptionsRef = useRef<Subscription[]>([]);
  useEffect(() => {
    subscriptionsRef.current = subscriptions;
  }, [subscriptions]);
  const [selectedServer, setSelectedServer] = useState<Server | null>(null);
  const [ruBypass, setRuBypass] = useState(false);
  const [killSwitch, setKillSwitch] = useState(false);
  const [bypassStatus, setBypassStatus] = useState("");
  const [busy, setBusy] = useState(false);
  const [geoipLoading, setGeoipLoading] = useState(false);
  const [geositeLoading, setGeositeLoading] = useState(false);
  const [updateChecking, setUpdateChecking] = useState(false);

  const [sheetOpen, setSheetOpen] = useState(false);
  const [sheetVisible, setSheetVisible] = useState(false);
  const [newUrl, setNewUrl] = useState("");
  const [addError, setAddError] = useState("");

  const [confirmOpen, setConfirmOpen] = useState(false);
  const [confirmVisible, setConfirmVisible] = useState(false);
  const [confirmTarget, setConfirmTarget] = useState<{ url: string; name: string } | null>(null);

  function pushToast(text: string, kind: "error" | "info" = "info") {
    const mine = { text, kind };
    setToast(mine);
    setTimeout(() => setToast((cur) => (cur === mine ? null : cur)), 4000);
  }

  async function refreshStatus() {
    setStatus(await invoke<ConnectionStatus>("get_status"));
  }

  async function persistSubscriptionMetas(subs: Subscription[]) {
    const metas: SubscriptionMeta[] = subs.map((s) => ({ url: s.url, name: s.name }));
    await invoke("set_setting", { key: "subscriptions", value: metas });
  }

  async function fetchAndStoreServers(meta: SubscriptionMeta): Promise<Subscription> {
    try {
      const servers = await invoke<Server[]>("fetch_servers", { url: meta.url });
      return { ...meta, servers, pings: {}, pinging: false, refreshing: false, error: "" };
    } catch (err) {
      return { ...meta, servers: [], pings: {}, pinging: false, refreshing: false, error: String(err) };
    }
  }

  // подгружаем сохранённые настройки при старте — тот же settings.json,
  // что у старого Python-приложения (core/settings.py в ветке main)
  useEffect(() => {
    (async () => {
      const saved = await invoke<Settings>("get_settings");
      setRuBypass(saved.ru_bypass_enabled);
      setKillSwitch(saved.kill_switch_enabled);
      const metas = saved.subscriptions ?? [];
      const loaded = await Promise.all(metas.map(fetchAndStoreServers));
      setSubscriptions(loaded);
      if (saved.last_selected_server) {
        for (const sub of loaded) {
          const found = sub.servers.find((s) => s.name === saved.last_selected_server);
          if (found) {
            setSelectedServer(found);
            break;
          }
        }
      }
    })();
  }, []);

  // движок может разорвать соединение сам, не по нашей команде
  // disconnect (например, сервер уронил QUIC-сессию) — без этого
  // слушателя UI продолжал бы показывать "подключено" при мёртвом
  // тоннеле, пока пользователь не откроет приложение заново
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    (async () => {
      unlisten = await listen("vpn-disconnected-unexpectedly", () => {
        pushToast("Соединение разорвано", "error");
        refreshStatus();
      });
    })();
    return () => unlisten?.();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // подписку на "выбрать сервер" из трея держим через ref на subscriptions
  // (она меняется часто — на каждый пинг/обновление), а на toggle и
  // ru_bypass переподписываемся, это происходит редко
  useEffect(() => {
    let unlistenToggle: (() => void) | undefined;
    let unlistenSelect: (() => void) | undefined;
    (async () => {
      unlistenToggle = await listen("tray-toggle-connection", () => {
        toggleConnection();
      });
      unlistenSelect = await listen<string>("tray-select-server", (event) => {
        const name = event.payload;
        for (const sub of subscriptionsRef.current) {
          const found = sub.servers.find((s) => s.name === name);
          if (found) {
            setSelectedServer(found);
            break;
          }
        }
      });
    })();
    return () => {
      unlistenToggle?.();
      unlistenSelect?.();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [status.connected, selectedServer, ruBypass]);

  // зеркалим текущее состояние в меню трея
  useEffect(() => {
    const servers = subscriptions.flatMap((s) => s.servers.map((srv) => srv.name));
    invoke("sync_tray", {
      connected: status.connected,
      currentServer: status.connected ? status.server_name : selectedServer?.name ?? null,
      servers,
    });
  }, [status, selectedServer, subscriptions]);

  function openAddSheet() {
    setNewUrl("");
    setAddError("");
    setSheetOpen(true);
    requestAnimationFrame(() => setSheetVisible(true));
  }

  function closeAddSheet() {
    setSheetVisible(false);
    setTimeout(() => setSheetOpen(false), 200);
  }

  async function addSubscriptionFromUrl(url: string): Promise<boolean> {
    if (subscriptions.some((s) => s.url === url)) {
      pushToast("такая подписка уже добавлена", "error");
      return false;
    }
    const meta: SubscriptionMeta = { url, name: subscriptionNameFromUrl(url) };
    const loaded = await fetchAndStoreServers(meta);
    if (loaded.error) {
      pushToast(loaded.error, "error");
      return false;
    }
    const next = [...subscriptions, loaded];
    setSubscriptions(next);
    await persistSubscriptionMetas(next);
    pushToast(`Подписка ${loaded.name} добавлена — ${loaded.servers.length} серверов`);
    return true;
  }

  async function confirmAddSubscription() {
    if (!newUrl.trim()) {
      setAddError("введи URL подписки");
      return;
    }
    setAddError("");
    if (await addSubscriptionFromUrl(newUrl.trim())) {
      closeAddSheet();
    }
  }

  async function pasteFromClipboard() {
    let text: string | null;
    try {
      text = await readText();
    } catch {
      pushToast("нет доступа к буферу обмена", "error");
      return;
    }
    if (!text || !text.trim()) {
      pushToast("буфер обмена пуст", "error");
      return;
    }
    await addSubscriptionFromUrl(text.trim());
  }

  async function refreshSubscription(url: string) {
    setSubscriptions((prev) => prev.map((s) => (s.url === url ? { ...s, refreshing: true } : s)));
    const current = subscriptions.find((s) => s.url === url);
    if (!current) return;
    const loaded = await fetchAndStoreServers({ url: current.url, name: current.name });
    if (loaded.error) {
      pushToast(loaded.error, "error");
      setSubscriptions((prev) => prev.map((s) => (s.url === url ? { ...s, refreshing: false } : s)));
      return;
    }
    setSubscriptions((prev) => prev.map((s) => (s.url === url ? loaded : s)));
  }

  async function deleteSubscription(url: string) {
    const next = subscriptions.filter((s) => s.url !== url);
    setSubscriptions(next);
    await persistSubscriptionMetas(next);
    if (selectedServer && !next.some((s) => s.servers.some((srv) => srv.name === selectedServer.name))) {
      setSelectedServer(null);
    }
  }

  function openDeleteConfirm(url: string, name: string) {
    setConfirmTarget({ url, name });
    setConfirmOpen(true);
    requestAnimationFrame(() => setConfirmVisible(true));
  }

  function closeDeleteConfirm() {
    setConfirmVisible(false);
    setTimeout(() => setConfirmOpen(false), 200);
  }

  async function confirmDeleteSubscription() {
    if (confirmTarget) {
      await deleteSubscription(confirmTarget.url);
    }
    closeDeleteConfirm();
  }

  async function pingSubscription(url: string) {
    setSubscriptions((prev) => prev.map((s) => (s.url === url ? { ...s, pinging: true } : s)));
    const sub = subscriptions.find((s) => s.url === url);
    if (!sub) return;
    const results = await invoke<PingResult[]>("ping_servers", { servers: sub.servers });
    const pings: Record<string, number | null> = {};
    for (const r of results) pings[r.name] = r.latency_ms;
    setSubscriptions((prev) => prev.map((s) => (s.url === url ? { ...s, pings, pinging: false } : s)));
  }

  async function toggleConnection() {
    if (busy) return;
    setBusy(true);
    try {
      if (status.connected) {
        await invoke("disconnect");
      } else {
        if (!selectedServer) {
          setBusy(false);
          return;
        }
        await invoke("connect", { server: selectedServer, ruBypass, killSwitch });
        await invoke("set_setting", { key: "last_selected_server", value: selectedServer.name });
      }
    } catch (err) {
      pushToast(String(err), "error");
    }
    setBusy(false);
    await refreshStatus();
  }

  async function checkAppUpdate() {
    setUpdateChecking(true);
    try {
      const r = await invoke<UpdateCheck>("check_app_update");
      pushToast(
        r.update_available
          ? `Доступна версия ${r.latest} — ${r.changelog}`
          : `У вас последняя версия (${r.current})`,
      );
    } catch (err) {
      pushToast(String(err), "error");
    }
    setUpdateChecking(false);
  }

  async function onRuBypassChange(checked: boolean) {
    setRuBypass(checked);
    await invoke("set_setting", { key: "ru_bypass_enabled", value: checked });
  }

  async function onKillSwitchChange(checked: boolean) {
    setKillSwitch(checked);
    await invoke("set_setting", { key: "kill_switch_enabled", value: checked });
  }

  return (
    <div className="window">
      <div className={`toast-banner ${toast ? "visible " + toast.kind : ""}`}>
        {toast && <div className="toast-banner-text">{toast.text}</div>}
      </div>

      {page === "home" ? (
        <main className="page">
          {subscriptions.length === 0 && (
            <div className="empty-placeholder">
              Нет подписок — добавь через кнопку «Добавить» снизу
            </div>
          )}

          {subscriptions.map((sub) => (
            <div key={sub.url}>
              <div className="card">
                <div className="list-row sub-header">
                  <span className="row-title">{sub.name}</span>
                  <div className="row-actions">
                    <button
                      className="icon-btn"
                      title="Обновить подписку"
                      onClick={() => refreshSubscription(sub.url)}
                      disabled={sub.refreshing}
                    >
                      {sub.refreshing ? <span className="spinner" /> : <RefreshIcon />}
                    </button>
                    <button
                      className="icon-btn"
                      title="Проверить пинг серверов"
                      onClick={() => pingSubscription(sub.url)}
                      disabled={sub.pinging}
                    >
                      {sub.pinging ? <span className="spinner" /> : <PingIcon />}
                    </button>
                    <button
                      className="icon-btn destructive"
                      title="Удалить подписку"
                      onClick={() => openDeleteConfirm(sub.url, sub.name)}
                    >
                      <TrashIcon />
                    </button>
                  </div>
                </div>
                {sub.servers.map((srv, i) => (
                  <div
                    key={i}
                    className={`list-row clickable ${selectedServer?.name === srv.name ? "selected" : ""}`}
                    onClick={() => !status.connected && setSelectedServer(srv)}
                  >
                    <span className="row-title">{srv.name}</span>
                    {sub.pings[srv.name] !== undefined && (
                      <span className="muted-note">
                        {sub.pings[srv.name] === null ? "—" : `${sub.pings[srv.name]} мс`}
                      </span>
                    )}
                  </div>
                ))}
              </div>
            </div>
          ))}
        </main>
      ) : (
        <main className="page">
          <div>
            <div className="group-title">Маршрутизация</div>
            <div className="card">
              <div className="list-row">
                <span className="row-title">
                  Российские сервисы напрямую
                  <br />
                  <span className="row-subtitle">По IP (geoip) и по списку доменов (geosite)</span>
                </span>
                <Switch checked={ruBypass} onChange={onRuBypassChange} disabled={status.connected} />
              </div>
              <div className="list-row">
                <span className="row-title">База IP-адресов России</span>
                <div className="row-actions">
                  <button
                    className="plain-button"
                    disabled={geoipLoading}
                    onClick={async () => {
                      setGeoipLoading(true);
                      try {
                        const r = await invoke<{ count: number; bytes: number }>("update_geoip");
                        setBypassStatus(`geoip: ${r.count} диапазонов, ${(r.bytes / 1024).toFixed(0)} КБ`);
                      } catch (err) {
                        pushToast(String(err), "error");
                      }
                      setGeoipLoading(false);
                    }}
                  >
                    {geoipLoading ? <span className="spinner" /> : "Обновить"}
                  </button>
                </div>
              </div>
              <div className="list-row">
                <span className="row-title">Список доменов российских сервисов</span>
                <div className="row-actions">
                  <button
                    className="plain-button"
                    disabled={geositeLoading}
                    onClick={async () => {
                      setGeositeLoading(true);
                      try {
                        const r = await invoke<{ count: number; bytes: number }>("update_geosite");
                        setBypassStatus(`geosite: ${r.count} доменов, ${(r.bytes / 1024).toFixed(0)} КБ`);
                      } catch (err) {
                        pushToast(String(err), "error");
                      }
                      setGeositeLoading(false);
                    }}
                  >
                    {geositeLoading ? <span className="spinner" /> : "Обновить"}
                  </button>
                </div>
              </div>
              {bypassStatus && (
                <div className="list-row">
                  <span className="muted-note">{bypassStatus}</span>
                </div>
              )}
            </div>
          </div>

          <div>
            <div className="group-title">Защита</div>
            <div className="card">
              <div className="list-row">
                <span className="row-title">
                  Kill Switch
                  <br />
                  <span className="row-subtitle">Блокировать трафик без VPN</span>
                </span>
                <Switch checked={killSwitch} onChange={onKillSwitchChange} disabled={status.connected} />
              </div>
            </div>
          </div>

          <div>
            <div className="group-title">О программе</div>
            <div className="card">
              <div className="list-row">
                <span className="row-title">vrox.vpn 4.0.0</span>
              </div>
              <div className="list-row">
                <button className="plain-button" disabled={updateChecking} onClick={checkAppUpdate}>
                  {updateChecking ? <span className="spinner" /> : "Проверить обновления"}
                </button>
              </div>
            </div>
          </div>

          <div>
            <div className="card">
              <div className="list-row">
                <button className="plain-button destructive-text" onClick={() => invoke("quit_app")}>
                  Закрыть полностью
                </button>
              </div>
            </div>
          </div>
        </main>
      )}

      {page === "home" && (
        <div className="bottom-action">
          {subscriptions.length === 0 ? (
            <div className="bottom-action-row">
              <button className="connect-button suggested" onClick={openAddSheet}>
                Добавить
              </button>
              <button className="connect-button outline" onClick={pasteFromClipboard}>
                Вставить из буфера
              </button>
            </div>
          ) : (
            <div className="bottom-action-row">
              <button
                className={`connect-button main ${status.connected ? "destructive" : "suggested"}`}
                onClick={toggleConnection}
                disabled={busy || (!status.connected && !selectedServer)}
              >
                {busy
                  ? status.connected
                    ? "Отключение…"
                    : "Подключение…"
                  : status.connected
                    ? `Отключиться (${status.server_name})`
                    : selectedServer
                      ? `Подключиться (${selectedServer.name})`
                      : "Выбери сервер"}
              </button>
              <button className="connect-button add-small" title="Добавить подписку" onClick={openAddSheet}>
                +
              </button>
            </div>
          )}
        </div>
      )}

      <nav className="view-switcher">
        <button className={page === "home" ? "active" : ""} onClick={() => setPage("home")}>
          <span className="icon">⌂</span>
          Главная
        </button>
        <button className={page === "settings" ? "active" : ""} onClick={() => setPage("settings")}>
          <span className="icon">⚙</span>
          Настройки
        </button>
      </nav>

      {sheetOpen && (
        <div className={`sheet-backdrop ${sheetVisible ? "visible" : ""}`} onClick={closeAddSheet}>
          <div className={`sheet ${sheetVisible ? "visible" : ""}`} onClick={(e) => e.stopPropagation()}>
            <div className="sheet-handle" />
            <h3>Добавить подписку</h3>
            <input
              className="text-input"
              value={newUrl}
              onChange={(e) => setNewUrl(e.currentTarget.value)}
              placeholder="URL подписки"
              autoFocus
            />
            {addError && <div className="banner error">{addError}</div>}
            <button className="connect-button suggested" onClick={confirmAddSubscription}>
              Добавить
            </button>
          </div>
        </div>
      )}

      {confirmOpen && confirmTarget && (
        <div className={`sheet-backdrop ${confirmVisible ? "visible" : ""}`} onClick={closeDeleteConfirm}>
          <div className={`sheet ${confirmVisible ? "visible" : ""}`} onClick={(e) => e.stopPropagation()}>
            <div className="sheet-handle" />
            <h3>Удалить подписку?</h3>
            <p className="sheet-text">{confirmTarget.name}</p>
            <div className="bottom-action-row">
              <button className="connect-button outline" onClick={closeDeleteConfirm}>
                Отмена
              </button>
              <button className="connect-button destructive" onClick={confirmDeleteSubscription}>
                Удалить
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

export default App;
