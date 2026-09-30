import { useRef, useState } from "react";
import type { Toast } from "@/types";

// Сколько уведомление висит, сколько длится анимация ухода и сколько
// видно одновременно (свежее сверху, старые вытесняются).
const TOAST_DURATION_MS = 4000;
const TOAST_LEAVE_MS = 200;
const MAX_VISIBLE_TOASTS = 2;

// Всплывающие уведомления в стиле push iOS (components/toast). Раньше —
// полоса над экраном, которая раздвигала интерфейс и её нельзя было закрыть.
export function useToast() {
  const [toasts, setToasts] = useState<Toast[]>([]);
  const nextId = useRef(0);

  function removeToast(id: number) {
    setToasts((prev) => prev.filter((toast) => toast.id !== id));
  }

  // сначала анимация ухода, потом удаление из списка
  function dismissToast(id: number) {
    setToasts((prev) => prev.map((toast) => (toast.id === id ? { ...toast, isLeaving: true } : toast)));
    setTimeout(() => removeToast(id), TOAST_LEAVE_MS);
  }

  function pushToast(text: string, kind: "error" | "info" = "info", detail?: string) {
    nextId.current += 1;
    const id = nextId.current;
    setToasts((prev) => [{ id, text, kind, detail, isLeaving: false }, ...prev].slice(0, MAX_VISIBLE_TOASTS));
    setTimeout(() => dismissToast(id), TOAST_DURATION_MS);
  }

  return { toasts, pushToast, dismissToast };
}
