import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";

export function useGeoUpdates(pushToast: (text: string, kind?: "error" | "info") => void) {
  const [geoipLoading, setGeoipLoading] = useState(false);
  const [geositeLoading, setGeositeLoading] = useState(false);
  const [bypassStatus, setBypassStatus] = useState("");

  async function updateGeoip() {
    setGeoipLoading(true);
    try {
      const r = await invoke<{ count: number; bytes: number }>("update_geoip");
      setBypassStatus(`geoip: ${r.count} ranges, ${(r.bytes / 1024).toFixed(0)} KB`);
    } catch (err) {
      pushToast(String(err), "error");
    }
    setGeoipLoading(false);
  }

  async function updateGeosite() {
    setGeositeLoading(true);
    try {
      const r = await invoke<{ count: number; bytes: number }>("update_geosite");
      setBypassStatus(`geosite: ${r.count} domains, ${(r.bytes / 1024).toFixed(0)} KB`);
    } catch (err) {
      pushToast(String(err), "error");
    }
    setGeositeLoading(false);
  }

  return { geoipLoading, geositeLoading, bypassStatus, updateGeoip, updateGeosite };
}
