import { useState } from "react";
import { AddTabEnum } from "@/components/add-sheet/add-tab";
import { useSheet } from "@/hooks/useSheet";
import { useAddSubscription } from "@/hooks/subscriptions";
import { useWireGuardForm } from "@/hooks/wireguard";
import type { useSubscriptions } from "@/hooks/subscriptions";
import type { useWireGuard } from "@/hooks/wireguard";

type TPushToast = (text: string, kind?: "error" | "info") => void;

// Шторка «+» экрана Nodes: вкладка подписки и вкладка WireGuard, у каждой
// своя форма; открытие сбрасывает обе.
export function useAddNode(
  subs: ReturnType<typeof useSubscriptions>,
  wireguard: ReturnType<typeof useWireGuard>,
  pushToast: TPushToast,
) {
  const sheet = useSheet();
  const [tab, setTab] = useState<AddTabEnum>(AddTabEnum.Subscription);
  const subscriptionForm = useAddSubscription(subs, pushToast);
  const wireguardForm = useWireGuardForm(wireguard, pushToast);

  function open() {
    subscriptionForm.reset();
    wireguardForm.reset();
    sheet.show();
  }

  return { sheet, tab, setTab, subscriptionForm, wireguardForm, open };
}
