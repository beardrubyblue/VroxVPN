import { createPortal } from "react-dom";

interface ScannerOverlayProps {
  onCancel: () => void;
}

// Рамка и кнопка отмены поверх камеры. Через портал в <body>: пока идёт
// скан, #root скрыт (класс is-scanning, App.css), а оверлей должен
// остаться видимым.
export function ScannerOverlay({ onCancel }: ScannerOverlayProps) {
  return createPortal(
    <div className="scanner-overlay" role="dialog" aria-label="Scan QR code">
      <div className="scanner-frame" aria-hidden="true" />
      <p className="scanner-hint">Point the camera at the QR code from wg-easy</p>
      <button className="scanner-cancel" onClick={onCancel}>
        Cancel
      </button>
    </div>,
    document.body,
  );
}
