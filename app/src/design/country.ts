// Код страны из имени сервера: первые 2 буквы латиницей в верхнем
// регистре, если они есть (наши имена серверов вида "NL · Amsterdam"
// или "Germany #2") — иначе первые 2 символа (FlagDot нарисует
// нейтральный серый, если код не из палитры).
export function countryCodeFromName(name: string): string {
  const m = name.match(/\b([A-Za-z]{2})\b/);
  return m ? m[1].toUpperCase() : name.slice(0, 2).toUpperCase();
}
