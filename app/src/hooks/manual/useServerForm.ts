import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { useManualServers } from "./useManualServers";
import { useQrScanner } from "./useQrScanner";

type TPushToast = (text: string, kind?: "error" | "info", detail?: string) => void;

// Вкладка «Server» шторки «+»: один сервер — ссылка hysteria2:// или .conf
// WireGuard (вставить, файл, QR). Протокол определяет Rust (manual/store.rs);
// пустое имя — имя из ссылки или хост сервера.
export function useServerForm(manual: ReturnType<typeof useManualServers>, pushToast: TPushToast) {
  const [name, setName] = useState("");
  const [text, setText] = useState("");
  const [error, setError] = useState("");
  const scanner = useQrScanner(pushToast);

  function reset() {
    setName("");
    setText("");
    setError("");
  }

  function acceptText(value: string) {
    setText(value);
    setError("");
  }

  async function pickFile() {
    try {
      const content = await invoke<string | null>("pick_config_file");
      if (content) acceptText(content);
    } catch (err) {
      pushToast(String(err), "error");
    }
  }

  async function scanQr() {
    const content = await scanner.scanQr();
    if (content) acceptText(content);
  }

  async function confirm(): Promise<boolean> {
    if (!text.trim()) {
      setError("Paste a link or a config, choose a file or scan a QR code");
      return false;
    }
    setError("");
    return manual.importText(name, text);
  }

  return { name, setName, text, setText: acceptText, error, reset, pickFile, scanQr, confirm, scanner };
}
