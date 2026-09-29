import { useState } from "react";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { useSheet } from "@/hooks/useSheet";
import type { useConnection } from "@/hooks/useConnection";
import type { useSubscriptions } from "./useSubscriptions";

type TPushToast = (text: string, kind?: "error" | "info") => void;
export type TSubscriptionSheetMode = "menu" | "rename" | "delete";

interface UseSubscriptionActionsArgs {
  subs: ReturnType<typeof useSubscriptions>;
  connection: ReturnType<typeof useConnection>;
  pushToast: TPushToast;
}

// Одна шторка на подписку, содержимое переключается режимом (меню →
// переименование / подтверждение удаления) — вместо трёх шторок, которые
// накладывались бы друг на друга во время 200мс анимации закрытия.
export function useSubscriptionActions({ subs, connection, pushToast }: UseSubscriptionActionsArgs) {
  const sheet = useSheet();
  const [mode, setMode] = useState<TSubscriptionSheetMode>("menu");
  const [targetUrl, setTargetUrl] = useState<string | null>(null);
  const [nameDraft, setNameDraft] = useState("");

  const target = subs.subscriptions.find((sub) => sub.url === targetUrl);
  const isTargetConnected =
    connection.status.connected && !!target?.servers.some((server) => server.name === connection.status.server_name);

  function openMenu(url: string) {
    setTargetUrl(url);
    setMode("menu");
    sheet.show();
  }

  function refresh() {
    if (target) subs.refresh(target.url);
    sheet.hide();
  }

  async function copyLink() {
    if (!target) return;
    try {
      await writeText(target.url);
      pushToast("Link copied");
    } catch {
      pushToast("No clipboard access", "error");
    }
    sheet.hide();
  }

  function startRename() {
    setNameDraft(target?.name ?? "");
    setMode("rename");
  }

  async function confirmRename() {
    const name = nameDraft.trim();
    if (target && name) await subs.rename(target.url, name);
    sheet.hide();
  }

  // Удаление подписки, к серверу которой мы подключены, сначала рвёт
  // тоннель — иначе VPN продолжал бы работать через сервер, которого
  // больше нет ни в одном списке.
  async function confirmDelete() {
    if (!target) return;
    if (isTargetConnected) await connection.toggleConnection();
    const selectedName = connection.selectedServer?.name;
    if (target.servers.some((server) => server.name === selectedName)) connection.setSelectedServer(null);
    await subs.remove(target.url);
    sheet.hide();
  }

  return {
    sheet,
    mode,
    setMode,
    target,
    isTargetConnected,
    nameDraft,
    setNameDraft,
    openMenu,
    refresh,
    copyLink,
    startRename,
    confirmRename,
    confirmDelete,
  };
}
