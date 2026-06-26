interface AddSubscriptionSheetProps {
  open: boolean;
  visible: boolean;
  url: string;
  onUrlChange: (url: string) => void;
  error: string;
  onConfirm: () => void;
  onClose: () => void;
}

export function AddSubscriptionSheet({
  open,
  visible,
  url,
  onUrlChange,
  error,
  onConfirm,
  onClose,
}: AddSubscriptionSheetProps) {
  if (!open) return null;

  return (
    <div className={`sheet-backdrop ${visible ? "visible" : ""}`} onClick={onClose}>
      <div className={`sheet ${visible ? "visible" : ""}`} onClick={(e) => e.stopPropagation()}>
        <div className="sheet-handle" />
        <h3>Добавить подписку</h3>
        <input
          className="text-input"
          value={url}
          onChange={(e) => onUrlChange(e.currentTarget.value)}
          placeholder="URL подписки"
          autoFocus
        />
        {error && <div className="banner error">{error}</div>}
        <button className="connect-button suggested" onClick={onConfirm}>
          Добавить
        </button>
      </div>
    </div>
  );
}
