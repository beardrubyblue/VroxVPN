interface SubscriptionDeleteConfirmProps {
  nodeCount: number;
  isConnected: boolean;
  onCancel: () => void;
  onConfirm: () => void;
  title?: string;
}

export function SubscriptionDeleteConfirm({
  nodeCount,
  isConnected,
  onCancel,
  onConfirm,
  title = "Delete subscription?",
}: SubscriptionDeleteConfirmProps) {
  return (
    <>
      <h3>{title}</h3>
      <p className="sheet-text">
        {nodeCount} {nodeCount === 1 ? "node" : "nodes"} will be removed from the list.
      </p>
      {isConnected && <p className="sub-sheet-warning">You're connected to one of its nodes — VPN will disconnect.</p>}
      <div className="sub-sheet-buttons">
        <button className="btn-secondary" onClick={onCancel}>
          Cancel
        </button>
        <button className="btn-danger" onClick={onConfirm}>
          Delete
        </button>
      </div>
    </>
  );
}
