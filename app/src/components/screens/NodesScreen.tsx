import { useMemo, useState } from "react";
import { Ic } from "@/design/icons";
import { FlagDot } from "@/design/brand";
import { countryCodeFromName } from "@/design/country";
import type { PingResult, Server } from "@/types";

interface NodesScreenProps {
  servers: Server[];
  pings: Record<string, PingResult>;
  activeName: string | undefined;
  onPick: (server: Server) => void;
  onBack: () => void;
  onAdd: () => void;
  onPaste: () => void;
}

// ScreenLocations (порт из дизайна) — список нод с поиском и фильтрами.
// Вшит в реальные подписки: servers — это плоский список всех серверов
// со всех подписок, pings — объединённая карта результатов пинга.
export function NodesScreen({ servers, pings, activeName, onPick, onBack, onAdd, onPaste }: NodesScreenProps) {
  const [query, setQuery] = useState("");
  const [tab, setTab] = useState<"all" | "fast">("all");

  const list = useMemo(() => {
    let r = servers.filter((s) => (query ? s.name.toLowerCase().includes(query.toLowerCase()) : true));
    if (tab === "fast") {
      r = [...r].sort((a, b) => {
        const pa = pings[a.name]?.latency_ms ?? Infinity;
        const pb = pings[b.name]?.latency_ms ?? Infinity;
        return pa - pb;
      });
    }
    return r;
  }, [servers, query, tab, pings]);

  return (
    <div className="vrox-screen">
      <div style={{ padding: "60px 20px 16px", display: "flex", justifyContent: "space-between", alignItems: "center" }}>
        <button onClick={onBack} className="btn-ghost" style={{ padding: 0 }}>
          ← Back
        </button>
        <div className="mono" style={{ fontSize: 11, color: "var(--fg-dim)", letterSpacing: "0.2em" }}>
          {servers.length} NODES
        </div>
      </div>
      <div style={{ padding: "0 20px" }}>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
          <h2 className="display" style={{ fontSize: 32, fontWeight: 600, margin: 0, letterSpacing: "-0.02em" }}>
            Nodes
          </h2>
          <div style={{ display: "flex", gap: 8 }}>
            <button
              onClick={onPaste}
              className="mono"
              style={{
                height: 36,
                padding: "0 14px",
                borderRadius: 999,
                border: "1px solid var(--line-strong)",
                background: "transparent",
                color: "var(--fg)",
                cursor: "pointer",
                fontSize: 10,
                letterSpacing: "0.12em",
              }}
            >
              PASTE
            </button>
            <button
              onClick={onAdd}
              aria-label="Add subscription"
              style={{
                width: 36,
                height: 36,
                borderRadius: 999,
                border: "none",
                background: "var(--fg)",
                color: "var(--bg)",
                cursor: "pointer",
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
              }}
            >
              <Ic.plus s={18} />
            </button>
          </div>
        </div>
        <div
          style={{
            marginTop: 18,
            display: "flex",
            alignItems: "center",
            gap: 10,
            background: "var(--bg-elev-1)",
            border: "1px solid var(--line)",
            borderRadius: 14,
            padding: "10px 14px",
          }}
        >
          <div style={{ color: "var(--fg-dim)" }}>
            <Ic.globe s={16} />
          </div>
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Search city or tag…"
            style={{
              flex: 1,
              background: "transparent",
              border: "none",
              outline: "none",
              color: "var(--fg)",
              fontSize: 14,
              fontFamily: "var(--font-ui)",
            }}
          />
        </div>
        <div style={{ display: "flex", gap: 6, marginTop: 12 }}>
          {(
            [
              ["all", "All"],
              ["fast", "Fastest"],
            ] as const
          ).map(([id, label]) => (
            <button
              key={id}
              onClick={() => setTab(id)}
              style={{
                padding: "7px 14px",
                borderRadius: 999,
                background: tab === id ? "var(--fg)" : "var(--bg-elev-2)",
                color: tab === id ? "var(--bg)" : "var(--fg-muted)",
                border: "1px solid var(--line)",
                cursor: "pointer",
                fontSize: 12,
                fontWeight: 500,
              }}
            >
              {label}
            </button>
          ))}
        </div>
      </div>

      <div className="scrollable" style={{ flex: 1, padding: "14px 20px 20px" }}>
        {list.length === 0 && (
          <div style={{ textAlign: "center", color: "var(--fg-dim)", fontSize: 13, padding: "40px 0" }}>
            {servers.length === 0 ? "No nodes — add a subscription first" : "Nothing matches your search"}
          </div>
        )}
        {list.map((s) => {
          const ping = pings[s.name];
          const ms = ping?.latency_ms ?? null;
          const active = s.name === activeName;
          // полоски сигнала: меньше пинг → больше полос (нет load в данных)
          const bars = ms === null ? 0 : ms < 40 ? 5 : ms < 70 ? 4 : ms < 110 ? 3 : ms < 170 ? 2 : 1;
          return (
            <button
              key={s.name}
              onClick={() => onPick(s)}
              style={{
                width: "100%",
                marginBottom: 8,
                padding: "14px 14px",
                display: "flex",
                alignItems: "center",
                gap: 12,
                background: active ? "var(--bg-elev-2)" : "var(--bg-elev-1)",
                border: `1px solid ${active ? "var(--fg)" : "var(--line)"}`,
                borderRadius: 16,
                color: "var(--fg)",
                cursor: "pointer",
                textAlign: "left",
              }}
            >
              <FlagDot code={countryCodeFromName(s.name)} size={32} />
              <div style={{ flex: 1, minWidth: 0 }}>
                <div style={{ fontSize: 14, fontWeight: 500, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                  {s.name}
                </div>
                <div
                  className="mono"
                  style={{
                    fontSize: 10,
                    color: "var(--fg-dim)",
                    marginTop: 3,
                    letterSpacing: "0.08em",
                    overflow: "hidden",
                    textOverflow: "ellipsis",
                    whiteSpace: "nowrap",
                  }}
                >
                  {s.host}
                </div>
              </div>
              <div style={{ textAlign: "right" }}>
                <div className="mono" style={{ fontSize: 13 }}>
                  {ms === null ? "—" : ms}
                  {ms !== null && <span style={{ fontSize: 10, color: "var(--fg-dim)" }}>ms</span>}
                </div>
                <div style={{ display: "flex", gap: 1, marginTop: 4, justifyContent: "flex-end" }}>
                  {[0, 1, 2, 3, 4].map((i) => (
                    <div
                      key={i}
                      style={{
                        width: 3,
                        height: 3 + i * 2,
                        borderRadius: 1,
                        background: i < bars ? "var(--fg)" : "var(--line-strong)",
                      }}
                    />
                  ))}
                </div>
              </div>
            </button>
          );
        })}
      </div>
    </div>
  );
}
