interface SubscriptionRenameFormProps {
  name: string;
  onNameChange: (name: string) => void;
  onCancel: () => void;
  onConfirm: () => void;
}

export function SubscriptionRenameForm({ name, onNameChange, onCancel, onConfirm }: SubscriptionRenameFormProps) {
  return (
    <form
      style={{ display: "flex", flexDirection: "column", gap: 14 }}
      onSubmit={(event) => {
        event.preventDefault();
        onConfirm();
      }}
    >
      <input
        className="text-input"
        value={name}
        onChange={(event) => onNameChange(event.currentTarget.value)}
        placeholder="Subscription name"
        aria-label="Subscription name"
        autoFocus
      />
      <div className="sub-sheet-buttons">
        <button type="button" className="btn-secondary" onClick={onCancel}>
          Cancel
        </button>
        <button type="submit" className="btn-primary" disabled={!name.trim()}>
          Save
        </button>
      </div>
    </form>
  );
}
