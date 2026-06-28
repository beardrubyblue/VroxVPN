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
        <h3>Delete subscription?</h3>
        <p className="sheet-text">{targetName}</p>
        <div style={{ display: "flex", gap: 10 }}>
          <button className="btn-secondary" style={{ flex: 1 }} onClick={onCancel}>
            Cancel
          </button>
          <button className="btn-danger" style={{ flex: 1 }} onClick={onConfirm}>
            Delete
          </button>
        </div>
      </div>
    </div>
  );
}
