// Определение платформы по userAgent webview — синхронно и без
// зависимостей (@tauri-apps/plugin-os тянет отдельный Rust-плагин ради
// одной проверки, а async-команда дала бы мелькание UI до ответа).
// На iOS (WKWebView) userAgent содержит iPhone/iPad; на десктопной
// macOS-сборке Tauri — обычный WebKit без них, так что специфичные для
// десктопа элементы там остаются.
export const isIOS =
  typeof navigator !== "undefined" &&
  (/iPhone|iPad|iPod/.test(navigator.userAgent) ||
    // iPadOS 13+ маскируется под Mac в UA — отличаем по наличию тач-ввода
    (/Macintosh/.test(navigator.userAgent) && "ontouchend" in document));

// Linux-сборка (десктоп): WireGuard там пока не поддерживается, импорт
// .conf скрыт. iOS-сборка на Mac — не Linux.
export const isLinux = typeof navigator !== "undefined" && /Linux/.test(navigator.userAgent) && !/Android/.test(navigator.userAgent);
