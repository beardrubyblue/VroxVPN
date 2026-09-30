import type { useAddSubscription, useWireGuardForm } from "@/hooks";
import { ScannerOverlay } from "./ScannerOverlay";
import { SubscriptionForm } from "./SubscriptionForm";
import { WireGuardForm } from "./WireGuardForm";
import { AddTabEnum } from "./add-tab";

interface AddNodeSheetProps {
  isOpen: boolean;
  isVisible: boolean;
  tab: AddTabEnum;
  onTabChange: (tab: AddTabEnum) => void;
  // на Linux WireGuard пока не поддерживается — вкладки нет
  canAddWireGuard: boolean;
  subscriptionForm: ReturnType<typeof useAddSubscription>;
  wireguardForm: ReturnType<typeof useWireGuardForm>;
  onClose: () => void;
}

// Шторка «+»: подписка hysteria2 или конфиг WireGuard/AmneziaWG.
export function AddNodeSheet(props: AddNodeSheetProps) {
  const { isOpen, isVisible, tab, onTabChange, canAddWireGuard, subscriptionForm, wireguardForm, onClose } = props;
  if (!isOpen) return null;
  const activeTab = canAddWireGuard ? tab : AddTabEnum.Subscription;

  return (
    <div className={isVisible ? "sheet-backdrop visible" : "sheet-backdrop"} onClick={onClose}>
      <div className={isVisible ? "sheet visible" : "sheet"} role="dialog" aria-label="Add" onClick={(event) => event.stopPropagation()}>
        <div className="sheet-handle" />
        <h3>Add</h3>
        {canAddWireGuard && (
          <div className="add-tabs" role="tablist">
            <button
              role="tab"
              aria-selected={activeTab === AddTabEnum.Subscription}
              className={activeTab === AddTabEnum.Subscription ? "add-tab active" : "add-tab"}
              onClick={() => onTabChange(AddTabEnum.Subscription)}
            >
              Subscription
            </button>
            <button
              role="tab"
              aria-selected={activeTab === AddTabEnum.WireGuard}
              className={activeTab === AddTabEnum.WireGuard ? "add-tab active" : "add-tab"}
              onClick={() => onTabChange(AddTabEnum.WireGuard)}
            >
              WireGuard
            </button>
          </div>
        )}
        {activeTab === AddTabEnum.Subscription && <SubscriptionForm form={subscriptionForm} onDone={onClose} />}
        {activeTab === AddTabEnum.WireGuard && <WireGuardForm form={wireguardForm} onDone={onClose} />}
      </div>
      {wireguardForm.scanner.isScanning && <ScannerOverlay onCancel={wireguardForm.scanner.cancelScan} />}
    </div>
  );
}
