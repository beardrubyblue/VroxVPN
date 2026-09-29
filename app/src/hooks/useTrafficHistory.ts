import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { buildMonthWindow } from "@/utils/traffic-history";
import type { DayTraffic } from "@/types";

// Файл истории обновляется раз в 5с (history.go / recorder.rs) — чаще
// перечитывать незачем.
const HISTORY_REFRESH_MS = 5000;

// История трафика по дням за месяц для экрана Stats. Читается, только
// пока экран открыт (хук живёт в StatsScreen).
export function useTrafficHistory() {
  const [days, setDays] = useState<DayTraffic[]>(() => buildMonthWindow([]));
  const [isLoaded, setIsLoaded] = useState(false);

  useEffect(() => {
    let isActive = true;
    async function load() {
      try {
        const history = await invoke<DayTraffic[]>("get_traffic_history");
        if (isActive) setDays(buildMonthWindow(history));
      } catch {
        // файла ещё нет / не читается — остаётся пустое окно
      }
      if (isActive) setIsLoaded(true);
    }
    load();
    const interval = setInterval(load, HISTORY_REFRESH_MS);
    return () => {
      isActive = false;
      clearInterval(interval);
    };
  }, []);

  return { days, isLoaded };
}
