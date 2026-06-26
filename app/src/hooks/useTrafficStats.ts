import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { TrafficDisplay, TrafficTotals } from "@/types";

// Опрос трафика раз в секунду, как core/stats.py в питон-версии — там
// дельта считалась в фоновом потоке Python, здесь её считает сам
// фронтенд между двумя последовательными опросами get_traffic_totals
// (бэкенд отдаёт только суммарные байты с начала тоннеля, см.
// commands.rs/engine::get_traffic_totals).
export function useTrafficStats(connected: boolean, pushToast: (text: string, kind?: "error" | "info") => void) {
  const [traffic, setTraffic] = useState<TrafficDisplay | null>(null);
  // 0, не null — карточка памяти видна постоянно, 0 — честное
  // отображение "тоннель не запущен, процесса нет", не "загрузка"
  const [memoryBytes, setMemoryBytes] = useState(0);

  useEffect(() => {
    // Сброс отображаемых значений при disconnect — НЕ через setState
    // здесь (react-hooks/set-state-in-effect: синхронный setState в
    // теле эффекта может вызвать каскадные ререндеры). traffic и так
    // скрыт условием `connected && traffic` в разметке — устаревшее
    // значение в state просто не показывается. memoryBytes показывается
    // ВСЕГДА — видимое значение вычисляет вызывающий код (см. App.tsx::
    // displayedMemoryBytes), не сброс состояния тут.
    if (!connected) {
      return;
    }
    let prev: { up: number; down: number; time: number } | null = null;
    let cancelled = false;
    const tick = async () => {
      try {
        const totals = await invoke<TrafficTotals>("get_traffic_totals");
        if (cancelled) return;
        const now = Date.now();
        if (prev) {
          const dt = (now - prev.time) / 1000;
          setTraffic({
            upSpeed: dt > 0 ? Math.max(0, (totals.upload_bytes - prev.up) / dt) : 0,
            downSpeed: dt > 0 ? Math.max(0, (totals.download_bytes - prev.down) / dt) : 0,
            totalUp: totals.upload_bytes,
            totalDown: totals.download_bytes,
          });
        } else {
          setTraffic({ upSpeed: 0, downSpeed: 0, totalUp: totals.upload_bytes, totalDown: totals.download_bytes });
        }
        prev = { up: totals.upload_bytes, down: totals.download_bytes, time: now };
        setMemoryBytes(totals.memory_bytes);
      } catch (err) {
        // временная диагностика: раньше ошибка тут проглатывалась молча
        // (предполагалось, что это просто "тоннель отключился между
        // опросами") — оказалось полезно увидеть текст реальной ошибки
        pushToast(`get_traffic_totals: ${String(err)}`, "error");
      }
    };
    tick();
    const interval = setInterval(tick, 1000);
    return () => {
      cancelled = true;
      clearInterval(interval);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [connected]);

  return { traffic, memoryBytes };
}
