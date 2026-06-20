import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

interface ConnectionStatus {
  connected: boolean;
  server_name: string | null;
}

function App() {
  const [status, setStatus] = useState<ConnectionStatus>({
    connected: false,
    server_name: null,
  });
  const [error, setError] = useState("");
  const [engineVersion, setEngineVersion] = useState("");
  // тестовый путь — конфиг, уже сгенерированный старым (Python) приложением
  // при реальном подключении; здесь просто проверяем, что наш Rust-движок
  // умеет привилегированно поднять/убить TUN по существующему конфигу
  const [configPath, setConfigPath] = useState("/tmp/vroxory-vpn/DE_Hysteria2.yaml");

  async function refreshStatus() {
    setStatus(await invoke<ConnectionStatus>("get_status"));
  }

  async function toggleConnection() {
    setError("");
    try {
      if (status.connected) {
        await invoke("disconnect");
      } else {
        await invoke("connect", { configPath });
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
        value={configPath}
        onChange={(e) => setConfigPath(e.currentTarget.value)}
        disabled={status.connected}
        style={{ width: "100%" }}
      />
      <button onClick={toggleConnection}>
        {status.connected ? "Отключиться" : "Подключиться"}
      </button>
      {error && <p className="error">{error}</p>}

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
