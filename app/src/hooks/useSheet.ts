import { useState } from "react";

// Открытие/закрытие модальной шторки (sheet) с задержкой на CSS-анимацию
// — раньше эта пара open/visible + RAF/setTimeout была продублирована
// для добавления подписки и подтверждения удаления (см. App.tsx до
// разбора). Сам момент показа/скрытия (что показывать в шторке, какие
// поля сбрасывать) остаётся за вызывающим кодом — этот хук только про
// механику open/visible.
export function useSheet() {
  const [open, setOpen] = useState(false);
  const [visible, setVisible] = useState(false);

  function show() {
    setOpen(true);
    requestAnimationFrame(() => setVisible(true));
  }

  function hide() {
    setVisible(false);
    setTimeout(() => setOpen(false), 200);
  }

  return { open, visible, show, hide };
}
