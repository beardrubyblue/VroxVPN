import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";

export function useSettings() {
  const [ruBypass, setRuBypass] = useState(false);
  const [killSwitch, setKillSwitch] = useState(false);

  async function onRuBypassChange(checked: boolean) {
    setRuBypass(checked);
    await invoke("set_setting", { key: "ru_bypass_enabled", value: checked });
  }

  async function onKillSwitchChange(checked: boolean) {
    setKillSwitch(checked);
    await invoke("set_setting", { key: "kill_switch_enabled", value: checked });
  }

  return { ruBypass, setRuBypass, killSwitch, setKillSwitch, onRuBypassChange, onKillSwitchChange };
}
