import { useState } from "react";
import type { Toast } from "@/types";

export function useToast() {
  const [toast, setToast] = useState<Toast | null>(null);

  function pushToast(text: string, kind: "error" | "info" = "info") {
    const mine = { text, kind };
    setToast(mine);
    setTimeout(() => setToast((cur) => (cur === mine ? null : cur)), 4000);
  }

  return { toast, pushToast };
}
