import React from "react";
import ReactDOM from "react-dom/client";
// Шрифты — собственные @font-face только с latin+cyrillic woff2 (см.
// fonts.css). НЕ через @fontsource/*.css: тот тянул бы лишние subset'ы
// и .woff-фолбэк (~1 МБ мёртвого груза на iOS).
import "./fonts.css";
import App from "./App";

// Запрет масштабирования жестами (WKWebView на iOS иногда позволяет
// pinch-зум даже при user-scalable=no в viewport): глушим Safari/WebKit
// gesture*-события и double-tap. Жест перетаскивания капсулы (ShieldScreen)
// использует pointer-события и этим не затрагивается.
for (const ev of ["gesturestart", "gesturechange", "gestureend"]) {
  document.addEventListener(ev, (e) => e.preventDefault(), { passive: false });
}
let lastTouchEnd = 0;
document.addEventListener(
  "touchend",
  (e) => {
    const now = Date.now();
    if (now - lastTouchEnd < 300) e.preventDefault(); // double-tap zoom
    lastTouchEnd = now;
  },
  { passive: false },
);

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
