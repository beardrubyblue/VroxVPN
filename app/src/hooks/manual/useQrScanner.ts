import { useState } from "react";
import { Format, cancel, checkPermissions, requestPermissions, scan } from "@tauri-apps/plugin-barcode-scanner";

type TPushToast = (text: string, kind?: "error" | "info") => void;

// Класс на <html>: камера рисуется ПОД прозрачной страницей
// (scan({ windowed: true })), поэтому на время скана фон страницы
// прозрачный, а интерфейс приложения скрыт — видна только рамка с кнопкой
// отмены (ScannerOverlay, через портал). См. App.css.
const SCANNING_CLASS = "is-scanning";

// Скан QR-кода с сервером (wg-easy показывает QR с .conf для каждого
// клиента; подойдёт и QR со ссылкой hysteria2://). windowed: true, а не полноэкранная камера плагина — у той нет
// кнопки отмены.
export function useQrScanner(pushToast: TPushToast) {
  const [isScanning, setIsScanning] = useState(false);

  async function hasCameraAccess(): Promise<boolean> {
    if ((await checkPermissions()) === "granted") return true;
    return (await requestPermissions()) === "granted";
  }

  // Текст из QR или null (отмена / нет доступа / ошибка — ошибка тостом).
  async function scanQr(): Promise<string | null> {
    try {
      if (!(await hasCameraAccess())) {
        pushToast("Camera access is off — allow it in Settings", "error");
        return null;
      }
      document.documentElement.classList.add(SCANNING_CLASS);
      setIsScanning(true);
      const scanned = await scan({ windowed: true, formats: [Format.QRCode] });
      return scanned.content;
    } catch (err) {
      const message = String(err);
      if (!/cancel/i.test(message)) pushToast(message, "error");
      return null;
    } finally {
      document.documentElement.classList.remove(SCANNING_CLASS);
      setIsScanning(false);
    }
  }

  async function cancelScan() {
    try {
      await cancel();
    } catch {
      // скан уже завершился
    }
  }

  return { isScanning, scanQr, cancelScan };
}
