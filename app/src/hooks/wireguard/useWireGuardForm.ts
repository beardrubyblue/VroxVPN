import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { useWireGuard } from "./useWireGuard";
import { useQrScanner } from "./useQrScanner";

type TPushToast = (text: string, kind?: "error" | "info") => void;

// Поле «имя» по умолчанию — Address клиента не информативен, поэтому
// берём хост из Endpoint (как у подписок — hostname).
function defaultNameFromConf(conf: string): string {
  const endpoint = /^\s*Endpoint\s*=\s*(.+)$/im.exec(conf)?.[1]?.trim() ?? "";
  return endpoint.replace(/:\d+$/, "").replace(/^\[|\]$/g, "") || "WireGuard";
}

// Форма импорта WireGuard в шторке «+»: текст конфига — вставкой, из файла
// (Rust pick_wireguard_file) или из QR.
export function useWireGuardForm(wireguard: ReturnType<typeof useWireGuard>, pushToast: TPushToast) {
  const [name, setName] = useState("");
  const [conf, setConf] = useState("");
  const [error, setError] = useState("");
  const scanner = useQrScanner(pushToast);

  function reset() {
    setName("");
    setConf("");
    setError("");
  }

  function acceptConf(text: string) {
    setConf(text);
    setError("");
    if (!name.trim()) setName(defaultNameFromConf(text));
  }

  async function pickFile() {
    try {
      const text = await invoke<string | null>("pick_wireguard_file");
      if (text) acceptConf(text);
    } catch (err) {
      pushToast(String(err), "error");
    }
  }

  async function scanQr() {
    const text = await scanner.scanQr();
    if (text) acceptConf(text);
  }

  async function confirm(): Promise<boolean> {
    if (!conf.trim()) {
      setError("Paste a config, choose a file or scan a QR code");
      return false;
    }
    setError("");
    return wireguard.importConf(name.trim() || defaultNameFromConf(conf), conf);
  }

  return { name, setName, conf, setConf: acceptConf, error, reset, pickFile, scanQr, confirm, scanner };
}
