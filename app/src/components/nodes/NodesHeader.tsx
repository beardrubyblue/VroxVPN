import { Ic } from "@/design/icons";

interface NodesHeaderProps {
  subscriptionCount: number;
  nodeCount: number;
  isRefreshing: boolean;
  onBack: () => void;
  onPaste: () => void;
  onRefreshAll: () => void;
  onAdd: () => void;
}

export function NodesHeader({
  subscriptionCount,
  nodeCount,
  isRefreshing,
  onBack,
  onPaste,
  onRefreshAll,
  onAdd,
}: NodesHeaderProps) {
  return (
    <>
      <div className="nodes-top">
        <button onClick={onBack} className="btn-ghost" style={{ padding: 0 }}>
          ← Back
        </button>
        <div className="nodes-counter mono">
          {subscriptionCount} SUBS · {nodeCount} NODES
        </div>
      </div>
      <div className="nodes-head">
        <div className="nodes-title-row">
          <h2 className="nodes-title display">Nodes</h2>
          <div className="nodes-actions">
            <button onClick={onPaste} className="pill-btn mono">
              PASTE
            </button>
            <button
              onClick={onRefreshAll}
              className="round-btn"
              aria-label="Update all subscriptions"
              disabled={isRefreshing || subscriptionCount === 0}
            >
              <span className={isRefreshing ? "icon spin" : "icon"} aria-hidden="true">
                <Ic.refresh s={16} />
              </span>
            </button>
            <button onClick={onAdd} className="round-btn solid" aria-label="Add subscription">
              <Ic.plus s={18} />
            </button>
          </div>
        </div>
      </div>
    </>
  );
}
