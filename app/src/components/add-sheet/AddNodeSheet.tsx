import type { useAddSubscription, useServerForm } from "@/hooks";
import { ScannerOverlay } from "./ScannerOverlay";
import { SubscriptionForm } from "./SubscriptionForm";
import { ServerForm } from "./ServerForm";
import { AddTabEnum } from "./add-tab";

interface AddNodeSheetProps {
  isOpen: boolean;
  isVisible: boolean;
  tab: AddTabEnum;
  onTabChange: (tab: AddTabEnum) => void;
  // файл и QR — только iOS (на Linux только вставка ссылки/конфига)
  canPickFileOrQr: boolean;
  subscriptionForm: ReturnType<typeof useAddSubscription>;
  serverForm: ReturnType<typeof useServerForm>;
  onClose: () => void;
}

// Шторка «+»: выбираешь источник — подписка или один сервер (ссылка
// hysteria2:// или .conf WireGuard/AmneziaWG, протокол определяется сам).
export function AddNodeSheet(props: AddNodeSheetProps) {
  const { isOpen, isVisible, tab, onTabChange, canPickFileOrQr, subscriptionForm, serverForm, onClose } = props;
  if (!isOpen) return null;

  return (
    <div className={isVisible ? "sheet-backdrop visible" : "sheet-backdrop"} onClick={onClose}>
      <div className={isVisible ? "sheet visible" : "sheet"} role="dialog" aria-label="Add" onClick={(event) => event.stopPropagation()}>
        <div className="sheet-handle" />
        <h3>Add</h3>
        <div className="add-tabs" role="tablist">
            <button
              role="tab"
              aria-selected={tab === AddTabEnum.Subscription}
              className={tab === AddTabEnum.Subscription ? "add-tab active" : "add-tab"}
              onClick={() => onTabChange(AddTabEnum.Subscription)}
            >
              Subscription
            </button>
            <button
              role="tab"
              aria-selected={tab === AddTabEnum.Server}
              className={tab === AddTabEnum.Server ? "add-tab active" : "add-tab"}
              onClick={() => onTabChange(AddTabEnum.Server)}
            >
              Server
            </button>
          </div>
        {tab === AddTabEnum.Subscription && <SubscriptionForm form={subscriptionForm} onDone={onClose} />}
        {tab === AddTabEnum.Server && <ServerForm form={serverForm} canPickFileOrQr={canPickFileOrQr} onDone={onClose} />}
      </div>
      {serverForm.scanner.isScanning && <ScannerOverlay onCancel={serverForm.scanner.cancelScan} />}
    </div>
  );
}
