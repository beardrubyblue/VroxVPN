import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { relaunch } from "@tauri-apps/plugin-process";
import type { UpdateCheck, UpdateInfo } from "@/types";

// На macOS обновления приходят через TestFlight (App Store Connect сам
// доставляет и ставит) — здесь только информируем о версии, без кнопки
// "Установить". На Linux .deb качаем и ставим сами (см.
// install_update_linux / auto_installable в app_update.rs).
export function useAppUpdate(pushToast: (text: string, kind?: "error" | "info") => void) {
  const [checking, setChecking] = useState(false);
  const [installing, setInstalling] = useState(false);
  const [info, setInfo] = useState<UpdateInfo | null>(null);

  async function check() {
    setChecking(true);
    setInfo(null);
    try {
      const r = await invoke<UpdateCheck>("check_app_update");
      if (r.update_available) {
        setInfo({
          version: r.latest,
          notes: r.changelog,
          downloadUrl: r.download_url,
          sha256: r.sha256,
          autoInstallable: r.auto_installable,
        });
        pushToast(`Доступна версия ${r.latest} — ${r.changelog}`);
      } else {
        pushToast(`У вас последняя версия (${r.current})`);
      }
    } catch (err) {
      pushToast(String(err), "error");
    }
    setChecking(false);
  }

  async function install() {
    if (!info || !info.autoInstallable) return;
    setInstalling(true);
    try {
      await invoke("install_update_linux", { downloadUrl: info.downloadUrl, sha256: info.sha256 });
      // dpkg уже заменил бинарник на диске — перезапуск подхватывает
      // новую версию без ручных действий пользователя
      await relaunch();
    } catch (err) {
      pushToast(String(err), "error");
      setInstalling(false);
    }
  }

  return { checking, installing, info, check, install };
}
