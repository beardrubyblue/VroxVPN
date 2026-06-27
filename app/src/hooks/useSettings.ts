import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";

// Дефолты для relay-лимитов iOS (см. settings.rs::defaults) — нужны
// здесь же, чтобы UI показывал разумные числа до первого get_settings
// (тот же подход, что и false/false выше для переключателей).
const DEFAULT_IDLE_TIMEOUT_SECONDS = 30;
const DEFAULT_MAX_TCP_CONNECTIONS = 64;
const DEFAULT_MAX_UDP_CONNECTIONS = 32;

export function useSettings() {
  const [ruBypass, setRuBypass] = useState(false);
  const [killSwitch, setKillSwitch] = useState(false);
  const [idleTimeoutSeconds, setIdleTimeoutSeconds] = useState(DEFAULT_IDLE_TIMEOUT_SECONDS);
  const [maxTcpConnections, setMaxTcpConnections] = useState(DEFAULT_MAX_TCP_CONNECTIONS);
  const [maxUdpConnections, setMaxUdpConnections] = useState(DEFAULT_MAX_UDP_CONNECTIONS);

  async function onRuBypassChange(checked: boolean) {
    setRuBypass(checked);
    await invoke("set_setting", { key: "ru_bypass_enabled", value: checked });
  }

  async function onKillSwitchChange(checked: boolean) {
    setKillSwitch(checked);
    await invoke("set_setting", { key: "kill_switch_enabled", value: checked });
  }

  async function onIdleTimeoutSecondsChange(value: number) {
    setIdleTimeoutSeconds(value);
    await invoke("set_setting", { key: "idle_timeout_seconds", value });
  }

  async function onMaxTcpConnectionsChange(value: number) {
    setMaxTcpConnections(value);
    await invoke("set_setting", { key: "max_tcp_connections", value });
  }

  async function onMaxUdpConnectionsChange(value: number) {
    setMaxUdpConnections(value);
    await invoke("set_setting", { key: "max_udp_connections", value });
  }

  return {
    ruBypass,
    setRuBypass,
    killSwitch,
    setKillSwitch,
    onRuBypassChange,
    onKillSwitchChange,
    idleTimeoutSeconds,
    setIdleTimeoutSeconds,
    maxTcpConnections,
    setMaxTcpConnections,
    maxUdpConnections,
    setMaxUdpConnections,
    onIdleTimeoutSecondsChange,
    onMaxTcpConnectionsChange,
    onMaxUdpConnectionsChange,
  };
}
