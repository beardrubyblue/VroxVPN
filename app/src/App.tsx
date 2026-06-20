import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
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

function App() {
  const [status, setStatus] = useState<ConnectionStatus>({
    connected: false,
    server_name: null,
  });
  const [error, setError] = useState("");
  const [engineVersion, setEngineVersion] = useState("");

  const [subUrl, setSubUrl] = useState("");
  const [servers, setServers] = useState<Server[]>([]);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [ruBypass, setRuBypass] = useState(false);
  const [bypassStatus, setBypassStatus] = useState("");

  async function refreshStatus() {
    setStatus(await invoke<ConnectionStatus>("get_status"));
  }

  async function loadServers() {
    setError("");
    try {
      const list = await invoke<Server[]>("fetch_servers", { url: subUrl });
      setServers(list);
      setSelectedIndex(0);
    } catch (err) {
      setError(String(err));
    }
  }

  async function toggleConnection() {
    setError("");
    try {
      if (status.connected) {
        await invoke("disconnect");
      } else {
        const server = servers[selectedIndex];
        if (!server) {
          setError("сначала загрузи список серверов");
          return;
        }
        await invoke("connect", { server, ruBypass });
      }
    } catch (err) {
      setError(String(err));
    }
    await refreshStatus();
  }

  return (
    <main className="container">
      <h1>vrox.vpn</h1>
      <p>{status.connected ? `Подключено: ${status.server_name}` : "Отключено"}</p>

      <input
        value={subUrl}
        onChange={(e) => setSubUrl(e.currentTarget.value)}
        placeholder="URL подписки"
        disabled={status.connected}
        style={{ width: "100%" }}
      />
      <button onClick={loadServers} disabled={status.connected}>
        Получить серверы
      </button>

      {servers.length > 0 && (
        <select
          value={selectedIndex}
          onChange={(e) => setSelectedIndex(Number(e.currentTarget.value))}
          disabled={status.connected}
        >
          {servers.map((srv, i) => (
            <option key={i} value={i}>
              {srv.name}
            </option>
          ))}
        </select>
      )}

      <label>
        <input
          type="checkbox"
          checked={ruBypass}
          onChange={(e) => setRuBypass(e.currentTarget.checked)}
          disabled={status.connected}
        />
        Российские сервисы напрямую (geoip + geosite)
      </label>

      <button onClick={toggleConnection}>
        {status.connected ? "Отключиться" : "Подключиться"}
      </button>
      {error && <p className="error">{error}</p>}

      <hr />
      <p>Базы для обхода (geoip/geosite):</p>
      <button
        onClick={async () => {
          setError("");
          try {
            const r = await invoke<{ count: number; bytes: number }>("update_geoip");
            setBypassStatus(`geoip: ${r.count} диапазонов, ${(r.bytes / 1024).toFixed(0)} КБ`);
          } catch (err) {
            setError(String(err));
          }
        }}
      >
        Обновить geoip
      </button>
      <button
        onClick={async () => {
          setError("");
          try {
            const r = await invoke<{ count: number; bytes: number }>("update_geosite");
            setBypassStatus(`geosite: ${r.count} доменов, ${(r.bytes / 1024).toFixed(0)} КБ`);
          } catch (err) {
            setError(String(err));
          }
        }}
      >
        Обновить geosite
      </button>
      {bypassStatus && <p>{bypassStatus}</p>}

      <hr />
      <p>Проверка sidecar-процесса vroxcore:</p>
      <button
        onClick={async () => setEngineVersion(await invoke<string>("engine_version"))}
      >
        Проверить версию ядра
      </button>
      {engineVersion && <pre>{engineVersion}</pre>}
    </main>
  );
}

export default App;
