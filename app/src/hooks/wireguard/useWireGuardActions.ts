import { useState } from "react";
import { useSheet } from "@/hooks/useSheet";
import type { useConnection } from "@/hooks/useConnection";
import type { useWireGuard } from "./useWireGuard";

export type TWireGuardSheetMode = "menu" | "rename" | "delete";

interface UseWireGuardActionsArgs {
  wireguard: ReturnType<typeof useWireGuard>;
  connection: ReturnType<typeof useConnection>;
}

// Меню «…» у WireGuard-узла: переименовать / удалить — одна шторка с
// режимами, как у подписок (useSubscriptionActions).
export function useWireGuardActions({ wireguard, connection }: UseWireGuardActionsArgs) {
  const sheet = useSheet();
  const [mode, setMode] = useState<TWireGuardSheetMode>("menu");
  const [targetId, setTargetId] = useState<string | null>(null);
  const [nameDraft, setNameDraft] = useState("");

  const target = wireguard.servers.find((server) => server.wireguard?.id === targetId);
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
    if (targetId && name) await wireguard.rename(targetId, name);
    sheet.hide();
  }

  // удаление узла, к которому подключены, сначала рвёт тоннель
  async function confirmDelete() {
    if (!target || !targetId) return;
    if (isTargetConnected) await connection.toggleConnection();
    if (connection.selectedServer?.name === target.name) connection.setSelectedServer(null);
    await wireguard.remove(targetId);
    sheet.hide();
  }

  return { sheet, mode, setMode, target, isTargetConnected, nameDraft, setNameDraft, openMenu, startRename, confirmRename, confirmDelete };
}
