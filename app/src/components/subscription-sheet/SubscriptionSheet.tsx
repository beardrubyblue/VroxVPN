import type { useSubscriptionActions } from "@/hooks";
import { SubscriptionDeleteConfirm } from "./SubscriptionDeleteConfirm";
import { SubscriptionMenu } from "./SubscriptionMenu";
import { SubscriptionRenameForm } from "./SubscriptionRenameForm";

interface SubscriptionSheetProps {
  actions: ReturnType<typeof useSubscriptionActions>;
}

// Шторка действий над подпиской — одна на все режимы, см.
// hooks/subscriptions/useSubscriptionActions.ts.
export function SubscriptionSheet({ actions }: SubscriptionSheetProps) {
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
          <div className="sub-sheet-url mono">{target.url}</div>
        </div>
        {mode === "menu" && (
          <SubscriptionMenu
            onRefresh={actions.refresh}
            onRename={actions.startRename}
            onCopyLink={actions.copyLink}
            onDelete={() => actions.setMode("delete")}
          />
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
            nodeCount={target.servers.length}
            isConnected={actions.isTargetConnected}
            onCancel={() => actions.setMode("menu")}
            onConfirm={actions.confirmDelete}
          />
        )}
      </div>
    </div>
  );
}
