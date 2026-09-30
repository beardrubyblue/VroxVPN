import { Ic } from "@/design/icons";

interface SubscriptionMenuProps {
  // нет — пункта нет (у WireGuard-узла нечего обновлять и нет ссылки)
  onRefresh?: () => void;
  onRename: () => void;
  onCopyLink?: () => void;
  onDelete: () => void;
  deleteLabel?: string;
}

export function SubscriptionMenu({ onRefresh, onRename, onCopyLink, onDelete, deleteLabel = "Delete subscription" }: SubscriptionMenuProps) {
  return (
    <div className="sub-menu" role="menu">
      {onRefresh && (
        <button className="sub-menu-item" role="menuitem" onClick={onRefresh}>
          <Ic.refresh s={20} /> Update now
        </button>
      )}
      <button className="sub-menu-item" role="menuitem" onClick={onRename}>
        <Ic.edit s={20} /> Rename
      </button>
      {onCopyLink && (
        <button className="sub-menu-item" role="menuitem" onClick={onCopyLink}>
          <Ic.copy s={20} /> Copy link
        </button>
      )}
      <button className="sub-menu-item danger" role="menuitem" onClick={onDelete}>
        <Ic.trash s={20} /> {deleteLabel}
      </button>
    </div>
  );
}
