import type { ReactNode } from "react";
import { Ic } from "@/design/icons";
import type { UpdateInfo } from "@/types";
import { isIOS } from "@/utils/platform";

export type Theme = "light" | "dark";

interface SettingsScreenProps {
  theme: Theme;
  setTheme: (t: Theme) => void;
  ruBypass: boolean;
  onRuBypassChange: (v: boolean) => void;
  killSwitch: boolean;
  onKillSwitchChange: (v: boolean) => void;
  connected: boolean;
  geoipLoading: boolean;
  onUpdateGeoip: () => void;
  geositeLoading: boolean;
  onUpdateGeosite: () => void;
  bypassStatus: string;
  idleTimeoutSeconds: number;
  onIdleTimeoutSecondsChange: (v: number) => void;
  maxTcpConnections: number;
  onMaxTcpConnectionsChange: (v: number) => void;
  maxUdpConnections: number;
  onMaxUdpConnectionsChange: (v: number) => void;
  updateChecking: boolean;
  onCheckUpdate: () => void;
  updateInfo: UpdateInfo | null;
  updateInstalling: boolean;
  onInstallUpdate: () => void;
  onQuit: () => void;
}

// ── строительные блоки (порт из дизайна) ──────────────────────
function SetGroup({ title, children }: { title: string; children: ReactNode }) {
  return (
    <div style={{ marginBottom: 22 }}>
      <div className="mono" style={{ fontSize: 10, color: "var(--fg-dim)", letterSpacing: "0.2em", marginBottom: 8, padding: "0 6px" }}>
        {title}
      </div>
      <div style={{ background: "var(--bg-elev-1)", border: "1px solid var(--line)", borderRadius: 18, overflow: "hidden" }}>
        {children}
      </div>
    </div>
  );
}

function SetRow({ title, sub, right, last }: { title: string; sub?: string; right?: ReactNode; last?: boolean }) {
  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        gap: 12,
        padding: "14px 16px",
        borderBottom: last ? "none" : "1px solid var(--line)",
      }}
    >
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ fontSize: 14, letterSpacing: "-0.01em" }}>{title}</div>
        {sub && <div className="mono" style={{ fontSize: 11, color: "var(--fg-dim)", marginTop: 3 }}>{sub}</div>}
      </div>
      {right}
    </div>
  );
}

function SetSwitch({ on, onChange, disabled }: { on: boolean; onChange: (v: boolean) => void; disabled?: boolean }) {
  return (
    <div
      onClick={() => !disabled && onChange(!on)}
      style={{
        width: 44,
        height: 26,
        borderRadius: 999,
        flexShrink: 0,
        background: on ? "var(--fg)" : "var(--line-strong)",
        padding: 2,
        cursor: disabled ? "default" : "pointer",
        opacity: disabled ? 0.5 : 1,
        transition: "background 0.2s",
      }}
    >
      <div
        style={{
          width: 22,
          height: 22,
          borderRadius: "50%",
          background: on ? "var(--bg)" : "var(--bg-elev-1)",
          transform: "translateX(" + (on ? 18 : 0) + "px)",
          transition: "transform 0.2s",
        }}
      />
    </div>
  );
}

function UpdateButton({ label = "UPDATE", solid, loading, onClick }: { label?: string; solid?: boolean; loading?: boolean; onClick?: () => void }) {
  const isSolid = solid && !loading;
  return (
    <button
      onClick={onClick}
      disabled={loading}
      className="mono"
      style={{
        flexShrink: 0,
        padding: "9px 14px",
        borderRadius: 999,
        cursor: loading ? "default" : "pointer",
        border: "1px solid " + (isSolid ? "transparent" : "var(--line-strong)"),
        background: isSolid ? "var(--fg)" : "transparent",
        color: isSolid ? "var(--bg)" : "var(--fg)",
        fontSize: 10,
        letterSpacing: "0.16em",
        fontWeight: 500,
        display: "flex",
        alignItems: "center",
        gap: 6,
        whiteSpace: "nowrap",
        transition: "all 0.2s",
      }}
    >
      {loading && (
        <span
          style={{
            width: 9,
            height: 9,
            borderRadius: "50%",
            border: "1.5px solid currentColor",
            borderTopColor: "transparent",
            display: "inline-block",
            animation: "vrox-spin 0.7s linear infinite",
          }}
        />
      )}
      {loading ? "UPDATING…" : label}
    </button>
  );
}

function NumInput({ value, onChange, disabled }: { value: number; onChange: (v: number) => void; disabled?: boolean }) {
  return (
    <input
      type="number"
      className="mono"
      value={value}
      disabled={disabled}
      onChange={(e) => onChange(Number(e.target.value))}
      style={{
        width: 72,
        flexShrink: 0,
        textAlign: "right",
        padding: "8px 10px",
        borderRadius: 12,
        border: "1px solid var(--line-strong)",
        background: "var(--bg-elev-2)",
        color: "var(--fg)",
        fontSize: 14,
        opacity: disabled ? 0.5 : 1,
      }}
    />
  );
}

export function SettingsScreen(p: SettingsScreenProps) {
  return (
    <div className="vrox-screen">
      <div style={{ padding: "60px 20px 8px" }}>
        <h2 className="display" style={{ fontSize: 32, fontWeight: 600, margin: 0, letterSpacing: "-0.02em" }}>
          Settings
        </h2>
      </div>
      <div className="scrollable" style={{ flex: 1, padding: "16px 20px 20px" }}>
        {/* Тема */}
        <div
          style={{
            background: "var(--bg-elev-1)",
            border: "1px solid var(--line)",
            borderRadius: 18,
            padding: 6,
            display: "flex",
            gap: 4,
            marginBottom: 18,
          }}
        >
          {(
            [
              ["light", <Ic.sun s={14} />, "Light"],
              ["dark", <Ic.moon s={14} />, "Dark"],
            ] as [Theme, ReactNode, string][]
          ).map(([id, ic, label]) => (
            <button
              key={id}
              onClick={() => p.setTheme(id)}
              style={{
                flex: 1,
                height: 40,
                borderRadius: 14,
                border: "none",
                cursor: "pointer",
                background: p.theme === id ? "var(--bg-elev-3)" : "transparent",
                color: "var(--fg)",
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
                gap: 8,
                fontSize: 13,
                fontWeight: 500,
              }}
            >
              {ic} {label}
            </button>
          ))}
        </div>

        <SetGroup title="PROTECTION">
          <SetRow
            title="Kill switch"
            sub="Block all traffic if the tunnel drops"
            right={<SetSwitch on={p.killSwitch} onChange={p.onKillSwitchChange} disabled={p.connected} />}
            last
          />
        </SetGroup>

        <SetGroup title="ROUTING">
          <SetRow
            title="Russian services direct"
            sub="Send RU traffic outside the tunnel"
            right={<SetSwitch on={p.ruBypass} onChange={p.onRuBypassChange} disabled={p.connected} />}
          />
          <SetRow
            title="Russia IP database"
            sub={p.bypassStatus && p.bypassStatus.startsWith("geoip") ? p.bypassStatus : "geoip · IP ranges"}
            right={<UpdateButton loading={p.geoipLoading} onClick={p.onUpdateGeoip} />}
          />
          <SetRow
            title="Russia domain list"
            sub={p.bypassStatus && p.bypassStatus.startsWith("geosite") ? p.bypassStatus : "geosite · domains"}
            right={<UpdateButton loading={p.geositeLoading} onClick={p.onUpdateGeosite} />}
            last
          />
        </SetGroup>

        {isIOS && (
          <SetGroup title="PERFORMANCE">
            <SetRow
              title="Idle timeout"
              sub="Seconds idle before close"
              right={<NumInput value={p.idleTimeoutSeconds} onChange={p.onIdleTimeoutSecondsChange} disabled={p.connected} />}
            />
            <SetRow
              title="Max TCP connections"
              right={<NumInput value={p.maxTcpConnections} onChange={p.onMaxTcpConnectionsChange} disabled={p.connected} />}
            />
            <SetRow
              title="Max UDP connections"
              right={<NumInput value={p.maxUdpConnections} onChange={p.onMaxUdpConnectionsChange} disabled={p.connected} />}
              last
            />
          </SetGroup>
        )}

        <SetGroup title="APP VERSION">
          <SetRow
            title="vrox.vpn 4.0.0"
            sub={p.updateInfo ? `Update available · ${p.updateInfo.version}` : undefined}
            right={
              p.updateInfo && p.updateInfo.autoInstallable ? (
                <UpdateButton label="UPDATE" solid loading={p.updateInstalling} onClick={p.onInstallUpdate} />
              ) : (
                <UpdateButton label={p.updateInfo ? "TESTFLIGHT" : "CHECK"} loading={p.updateChecking} onClick={p.onCheckUpdate} />
              )
            }
            last
          />
        </SetGroup>

        {/* Quit — только на десктопе (на iOS приложения не закрывают себя
            программно: против Apple HIG, риск отклонения в App Store) */}
        {!isIOS && (
          <>
            <button
              onClick={p.onQuit}
              style={{
                marginTop: 4,
                width: "100%",
                height: 50,
                background: "transparent",
                border: "1px solid var(--line-strong)",
                borderRadius: 14,
                color: "var(--danger)",
                cursor: "pointer",
                fontSize: 14,
                fontWeight: 500,
                letterSpacing: "-0.01em",
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
                gap: 8,
              }}
            >
              <Ic.power s={17} sw={2} /> Quit completely
            </button>
            <div
              className="mono"
              style={{ fontSize: 10, color: "var(--fg-dim)", textAlign: "center", marginTop: 10, letterSpacing: "0.16em" }}
            >
              STOPS THE TUNNEL &amp; CLOSES THE APP
            </div>
          </>
        )}
      </div>
    </div>
  );
}
