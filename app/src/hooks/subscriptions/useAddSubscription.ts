import { useState } from "react";
import { readText } from "@tauri-apps/plugin-clipboard-manager";
import type { useSubscriptions } from "./useSubscriptions";

type TPushToast = (text: string, kind?: "error" | "info") => void;

// Добавление подписки: поле URL во вкладке «Subscription» шторки «+» и
// кнопка PASTE (сразу из буфера обмена). Шторкой управляет экран Nodes —
// она общая с импортом WireGuard.
export function useAddSubscription(subs: ReturnType<typeof useSubscriptions>, pushToast: TPushToast) {
  const [url, setUrl] = useState("");
  const [error, setError] = useState("");

  function reset() {
    setUrl("");
    setError("");
  }

  async function confirm(): Promise<boolean> {
    if (!url.trim()) {
      setError("Enter a subscription URL");
      return false;
    }
    setError("");
    return subs.addFromUrl(url.trim());
  }

  async function paste() {
    let text: string | null;
    try {
      text = await readText();
    } catch {
      pushToast("No clipboard access", "error");
      return;
    }
    if (!text || !text.trim()) {
      pushToast("Clipboard is empty", "error");
      return;
    }
    await subs.addFromUrl(text.trim());
  }

  return { url, setUrl, error, reset, confirm, paste };
}
