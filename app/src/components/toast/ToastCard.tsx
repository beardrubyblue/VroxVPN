import { useRef, useState } from "react";
import type { PointerEvent } from "react";
import { Ic } from "@/design/icons";
import type { Toast } from "@/types";

// Смахнуть вверх больше чем на столько — закрыть; меньше — это тап.
const SWIPE_DISMISS_PX = 24;
const TAP_SLOP_PX = 4;

interface ToastCardProps {
  toast: Toast;
  isBehind: boolean;
  onDismiss: (id: number) => void;
}

// Карточка уведомления: тап или смахивание вверх закрывают её.
export function ToastCard({ toast, isBehind, onDismiss }: ToastCardProps) {
  const startY = useRef<number | null>(null);
  const [dragY, setDragY] = useState(0);

  function onPointerDown(event: PointerEvent<HTMLButtonElement>) {
    event.currentTarget.setPointerCapture(event.pointerId);
    startY.current = event.clientY;
  }

  function onPointerMove(event: PointerEvent<HTMLButtonElement>) {
    if (startY.current === null) return;
    setDragY(Math.min(0, event.clientY - startY.current)); // только вверх
  }

  function onPointerUp() {
    const isSwipe = dragY < -SWIPE_DISMISS_PX;
    const isTap = Math.abs(dragY) < TAP_SLOP_PX;
    startY.current = null;
    setDragY(0);
    if (isSwipe || isTap) onDismiss(toast.id);
  }

  const className = ["toast-card", toast.kind, isBehind && "behind", toast.isLeaving && "leaving"].filter(Boolean).join(" ");

  return (
    <button
      className={className}
      style={dragY ? { transform: `translateY(${dragY}px)`, transition: "none" } : undefined}
      role={toast.kind === "error" ? "alert" : "status"}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onPointerCancel={onPointerUp}
      aria-label={`${toast.text}. Dismiss`}
    >
      <span className="toast-icon" aria-hidden="true">
        {toast.kind === "error" ? <Ic.alert s={20} /> : <Ic.check s={20} />}
      </span>
      <span className="toast-body">
        <span className="toast-title">{toast.text}</span>
        {toast.detail && <span className="toast-detail">{toast.detail}</span>}
      </span>
    </button>
  );
}
