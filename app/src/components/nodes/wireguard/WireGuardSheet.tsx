import { SubscriptionDeleteConfirm, SubscriptionMenu, SubscriptionRenameForm } from "@/components/subscription-sheet";
import type { useWireGuardActions } from "@/hooks";

interface WireGuardSheetProps {
  actions: ReturnType<typeof useWireGuardActions>;
}

// Шторка меню WireGuard-узла: переименовать / удалить (одна шторка с
// режимами, те же формы, что у подписок).
export function WireGuardSheet({ actions }: WireGuardSheetProps) {
  const { sheet, mode, target } = actions;
  if (!sheet.open || !target) return null;

  return (
    <div className={sheet.visible ? "sheet-backdrop visible" : "sheet-backdrop"} onClick={sheet.hide}>
      <div
        className={sheet.visible ? "sheet visible" : "sheet"}
        role="dialog"
        aria-label={target.name}
        onClick={(event) => event.stopPropagation()}
      >
        <div className="sheet-handle" />
        <div className="sub-sheet-head">
          <div className="sub-sheet-name mono">{target.name}</div>
          <div className="sub-sheet-url mono">
            WireGuard · {target.host}:{target.port}
          </div>
        </div>
        {mode === "menu" && (
          <SubscriptionMenu onRename={actions.startRename} onDelete={() => actions.setMode("delete")} deleteLabel="Delete config" />
        )}
        {mode === "rename" && (
          <SubscriptionRenameForm
            name={actions.nameDraft}
            onNameChange={actions.setNameDraft}
            onCancel={() => actions.setMode("menu")}
            onConfirm={actions.confirmRename}
          />
        )}
        {mode === "delete" && (
          <SubscriptionDeleteConfirm
            title="Delete WireGuard config?"
            nodeCount={1}
            isConnected={actions.isTargetConnected}
            onCancel={() => actions.setMode("menu")}
            onConfirm={actions.confirmDelete}
          />
        )}
      </div>
    </div>
  );
}
