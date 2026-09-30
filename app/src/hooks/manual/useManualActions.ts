import { useState } from "react";
import { useSheet } from "@/hooks/useSheet";
import type { useConnection } from "@/hooks/useConnection";
import type { useManualServers } from "./useManualServers";

export type TManualSheetMode = "menu" | "rename" | "delete";

interface UseManualActionsArgs {
  manual: ReturnType<typeof useManualServers>;
  connection: ReturnType<typeof useConnection>;
}

// Меню «…» у узла, добавленного вручную: переименовать / удалить — одна
// шторка с режимами, как у подписок (useSubscriptionActions).
export function useManualActions({ manual, connection }: UseManualActionsArgs) {
  const sheet = useSheet();
  const [mode, setMode] = useState<TManualSheetMode>("menu");
  const [targetId, setTargetId] = useState<string | null>(null);
  const [nameDraft, setNameDraft] = useState("");

  const target = manual.servers.find((server) => server.manual_id === targetId);
  const isTargetConnected = connection.status.connected && target?.name === connection.status.server_name;

  function openMenu(id: string) {
    setTargetId(id);
    setMode("menu");
    sheet.show();
  }

  function startRename() {
    setNameDraft(target?.name ?? "");
    setMode("rename");
  }

  async function confirmRename() {
    const name = nameDraft.trim();
    if (targetId && name) await manual.rename(targetId, name);
    sheet.hide();
  }

  // удаление узла, к которому подключены, сначала рвёт тоннель
  async function confirmDelete() {
    if (!target || !targetId) return;
    if (isTargetConnected) await connection.toggleConnection();
    if (connection.selectedServer?.name === target.name) connection.setSelectedServer(null);
    await manual.remove(targetId);
    sheet.hide();
  }

  return { sheet, mode, setMode, target, isTargetConnected, nameDraft, setNameDraft, openMenu, startRename, confirmRename, confirmDelete };
}
