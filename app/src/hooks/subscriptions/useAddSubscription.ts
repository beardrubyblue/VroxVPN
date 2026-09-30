import { useState } from "react";
import type { useSubscriptions } from "./useSubscriptions";

// Вкладка «Subscription» шторки «+»: поле URL подписки. Шторкой и кнопкой
// PASTE управляет useAddNode — они общие с вкладкой «Server».
export function useAddSubscription(subs: ReturnType<typeof useSubscriptions>) {
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

  return { url, setUrl, error, reset, confirm };
}
