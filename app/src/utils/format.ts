// Бюджет Apple для NE-расширений на iOS (~50МБ) — на macOS жёсткого
// задокументированного лимита нет, но держим тот же ориентир в UI как
// цель оптимизации (см. docs/ARCHITECTURE.md, раздел про статистику
// трафика/память).
export const MEMORY_BUDGET_BYTES = 50 * 1024 * 1024;

export function formatSpeed(bytesPerSec: number): string {
  if (bytesPerSec < 1024) return `${Math.round(bytesPerSec)} Б/с`;
  if (bytesPerSec < 1024 * 1024) return `${Math.round(bytesPerSec / 1024)} КБ/с`;
  return `${(bytesPerSec / (1024 * 1024)).toFixed(1)} МБ/с`;
}

export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} Б`;
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} КБ`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} МБ`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} ГБ`;
}

export function subscriptionNameFromUrl(url: string): string {
  try {
    return new URL(url).hostname;
  } catch {
    return url;
  }
}
