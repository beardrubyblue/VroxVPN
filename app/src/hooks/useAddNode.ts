import { useState } from "react";
import { readText } from "@tauri-apps/plugin-clipboard-manager";
import { AddTabEnum } from "@/components/add-sheet/add-tab";
import { useSheet } from "@/hooks/useSheet";
import { useAddSubscription } from "@/hooks/subscriptions";
import { useServerForm } from "@/hooks/manual";
import type { useSubscriptions } from "@/hooks/subscriptions";
import type { useManualServers } from "@/hooks/manual";

type TPushToast = (text: string, kind?: "error" | "info", detail?: string) => void;

// Текст одного сервера (ссылка hysteria2:// или .conf WireGuard), а не
// ссылка на подписку — то же правило, что в Rust manual/store.rs.
function isSingleServer(text: string): boolean {
  return text.startsWith("hysteria2://") || text.includes("[Interface]");
}

// Шторка «+» экрана Nodes (вкладки Subscription / Server) и кнопка PASTE,
// которая сама понимает, что в буфере: подписка или один сервер.
export function useAddNode(
  subs: ReturnType<typeof useSubscriptions>,
  manual: ReturnType<typeof useManualServers>,
  pushToast: TPushToast,
) {
  const sheet = useSheet();
  const [tab, setTab] = useState<AddTabEnum>(AddTabEnum.Subscription);
  const subscriptionForm = useAddSubscription(subs);
  const serverForm = useServerForm(manual, pushToast);

  function open() {
    subscriptionForm.reset();
    serverForm.reset();
    sheet.show();
  }

  async function paste() {
    let text: string | null;
    try {
      text = await readText();
    } catch {
      pushToast("No clipboard access", "error");
      return;
    }
    const value = text?.trim();
    if (!value) {
      pushToast("Clipboard is empty", "error");
      return;
    }
    if (isSingleServer(value)) await manual.importText("", value);
    else await subs.addFromUrl(value);
  }

  return { sheet, tab, setTab, subscriptionForm, serverForm, open, paste };
}
