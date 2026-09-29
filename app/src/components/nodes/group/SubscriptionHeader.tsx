import { Ic } from "@/design/icons";
import { formatSubscriptionMeta, usagePercent } from "@/utils/subscription-format";
import type { Subscription } from "@/types";

interface SubscriptionHeaderProps {
  subscription: Subscription;
  isOpen: boolean;
  onToggle: () => void;
  onRefresh: () => void;
  onMenu: () => void;
}

export function SubscriptionHeader({ subscription, isOpen, onToggle, onRefresh, onMenu }: SubscriptionHeaderProps) {
  const percent = usagePercent(subscription.usage);

  return (
    <div className="group-head">
      <button className="group-toggle" onClick={onToggle} aria-expanded={isOpen}>
        <span className={isOpen ? "group-chevron open" : "group-chevron"} aria-hidden="true">
          <Ic.chevron s={14} sw={2} />
        </span>
        <span className="group-info">
          <span className="group-title mono">
            {subscription.name}
          </span>
          <span className={subscription.error ? "group-meta mono error" : "group-meta mono"}>
            {formatSubscriptionMeta(subscription)}
          </span>
          {percent !== null && (
            <span className="group-quota" aria-label={`${percent}% of traffic used`}>
              <span style={{ width: `${percent}%` }} />
            </span>
          )}
        </span>
      </button>
      <button
        className="round-btn small"
        onClick={onRefresh}
        disabled={subscription.refreshing}
        aria-label={`Update ${subscription.name}`}
      >
        <span className={subscription.refreshing ? "icon spin" : "icon"} aria-hidden="true">
          <Ic.refresh s={14} />
        </span>
      </button>
      <button className="round-btn small" onClick={onMenu} aria-label={`${subscription.name} options`}>
        <Ic.more s={16} />
      </button>
    </div>
  );
}
