import type { Toast } from "@/types";

interface ToastBannerProps {
  toast: Toast | null;
}

export function ToastBanner({ toast }: ToastBannerProps) {
  return (
    <div className={`toast-banner ${toast ? "visible " + toast.kind : ""}`}>
      {toast && <div className="toast-banner-text">{toast.text}</div>}
    </div>
  );
}
