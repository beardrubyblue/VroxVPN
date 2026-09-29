import { useState } from "react";
import { readText } from "@tauri-apps/plugin-clipboard-manager";
import { useSheet } from "@/hooks/useSheet";
import type { useSubscriptions } from "./useSubscriptions";

type TPushToast = (text: string, kind?: "error" | "info") => void;

// Добавление подписки: шторка с полем URL и кнопка PASTE (сразу из
// буфера обмена). Раньше жило в App.tsx.
export function useAddSubscription(subs: ReturnType<typeof useSubscriptions>, pushToast: TPushToast) {
  const sheet = useSheet();
  const [url, setUrl] = useState("");
  const [error, setError] = useState("");

  function open() {
    setUrl("");
    setError("");
    sheet.show();
  }

  async function confirm() {
    if (!url.trim()) {
      setError("Enter a subscription URL");
      return;
    }
    setError("");
    if (await subs.addFromUrl(url.trim())) sheet.hide();
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

  return { sheet, url, setUrl, error, open, confirm, paste };
}
