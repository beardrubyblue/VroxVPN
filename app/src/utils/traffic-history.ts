import type { DayTraffic } from "@/types";

// Ровно месяц — по столбику на день (как и хранит файл истории, см.
// traffic_history.rs / history.go).
export const HISTORY_DAYS = 30;

const MONTHS = ["JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC"];

function localDateKey(date: Date): string {
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${date.getFullYear()}-${month}-${day}`;
}

// Окно последних HISTORY_DAYS дней, от старого к сегодняшнему; дни без
// трафика — нулевые (в файле их нет).
export function buildMonthWindow(days: DayTraffic[], today = new Date()): DayTraffic[] {
  const byDate = new Map(days.map((day) => [day.date, day]));
  return Array.from({ length: HISTORY_DAYS }, (_, index) => {
    const date = new Date(today.getFullYear(), today.getMonth(), today.getDate() - (HISTORY_DAYS - 1 - index));
    const key = localDateKey(date);
    return byDate.get(key) ?? { date: key, upload: 0, download: 0 };
  });
}

// «29 SEP» — в стиле моноширинных подписей экрана.
export function formatDayLabel(date: string): string {
  const [, month, day] = date.split("-").map(Number);
  return `${day} ${MONTHS[month - 1]}`;
}
