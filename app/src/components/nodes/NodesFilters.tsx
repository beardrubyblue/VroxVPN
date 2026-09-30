import { Ic } from "@/design/icons";
import { NodesTabEnum } from "./filter-nodes";

const TABS = [
  [NodesTabEnum.All, "All"],
  [NodesTabEnum.Fastest, "Fastest"],
  [NodesTabEnum.Hysteria2, "Hysteria2"],
  [NodesTabEnum.WireGuard, "WireGuard"],
] as const;

interface NodesFiltersProps {
  query: string;
  onQueryChange: (query: string) => void;
  tab: NodesTabEnum;
  onTabChange: (tab: NodesTabEnum) => void;
}

export function NodesFilters({ query, onQueryChange, tab, onTabChange }: NodesFiltersProps) {
  return (
    <div className="nodes-head">
      <label className="nodes-search">
        <span className="icon" aria-hidden="true">
          <Ic.globe s={16} />
        </span>
        <input
          value={query}
          onChange={(event) => onQueryChange(event.target.value)}
          placeholder="Search city or tag…"
          aria-label="Search nodes"
        />
      </label>
      <div className="nodes-tabs" role="tablist">
        {TABS.map(([id, label]) => (
          <button
            key={id}
            role="tab"
            aria-selected={tab === id}
            onClick={() => onTabChange(id)}
            className={tab === id ? "nodes-tab active" : "nodes-tab"}
          >
            {label}
          </button>
        ))}
      </div>
    </div>
  );
}
