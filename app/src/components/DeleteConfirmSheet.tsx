interface DeleteConfirmSheetProps {
  open: boolean;
  visible: boolean;
  targetName: string | undefined;
  onCancel: () => void;
  onConfirm: () => void;
}

export function DeleteConfirmSheet({ open, visible, targetName, onCancel, onConfirm }: DeleteConfirmSheetProps) {
  if (!open || targetName === undefined) return null;

  return (
    <div className={`sheet-backdrop ${visible ? "visible" : ""}`} onClick={onCancel}>
      <div className={`sheet ${visible ? "visible" : ""}`} onClick={(e) => e.stopPropagation()}>
        <div className="sheet-handle" />
        <h3>Удалить подписку?</h3>
        <p className="sheet-text">{targetName}</p>
        <div className="bottom-action-row">
          <button className="connect-button outline" onClick={onCancel}>
            Отмена
          </button>
          <button className="connect-button destructive" onClick={onConfirm}>
            Удалить
          </button>
        </div>
      </div>
    </div>
  );
}
