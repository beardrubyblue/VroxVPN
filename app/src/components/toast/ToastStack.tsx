import type { Toast } from "@/types";
import { ToastCard } from "./ToastCard";

interface ToastStackProps {
  toasts: Toast[];
  onDismiss: (id: number) => void;
}

// Стопка уведомлений сверху экрана: свежее первым, старое под ним.
export function ToastStack({ toasts, onDismiss }: ToastStackProps) {
  return (
    <div className="toast-stack" aria-live="polite">
      {toasts.map((toast, index) => (
        <ToastCard key={toast.id} toast={toast} isBehind={index > 0} onDismiss={onDismiss} />
      ))}
    </div>
  );
}
