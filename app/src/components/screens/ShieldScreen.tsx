import { useEffect, useRef, useState } from "react";
import { Ic } from "@/design/icons";
import { VroxWordmark, FlagDot } from "@/design/brand";
import { countryCodeFromName } from "@/design/country";
import type { Server } from "@/types";

// Расстояние от верха капсулы до верха стрелки-подсказки над ней.
const ARROW_GAP = 48;

interface ShieldScreenProps {
  connected: boolean;
  busy: boolean;
  server: Server | null;
  onToggle: () => void;
  onOpenLocations: () => void;
}

// HomeDock (порт из дизайна, секция 03) — подключение жестом: тянешь
// капсулу страны ВВЕРХ в круглый слот по центру; при стыковке канал
// активируется (морфинг pill → disc + замок + пульс-кольца). Тяни вниз,
// чтобы отключить. Тап по капсуле/слоту — тоже переключает.
export function ShieldScreen({ connected, busy, server, onToggle, onOpenLocations }: ShieldScreenProps) {
  const stageRef = useRef<HTMLDivElement>(null);
  const [drag, setDrag] = useState(false);
  const [progress, setProgress] = useState(connected ? 1 : 0);
  const slotRel = 0.5;
  const homeRel = 0.82;
  const clamp = (v: number, a: number, b: number) => Math.max(a, Math.min(b, v));
  const downY = useRef(0);
  const moved = useRef(false);

  // Пока не тащим — прогресс жёстко следует за реальным состоянием
  // соединения (в т.ч. внешние разрывы тоннеля). Это синхронизация
  // внешней системы (движок VPN) в локальное анимационное состояние —
  // редкий легитимный случай setState в эффекте.
  useEffect(() => {
    // eslint-disable-next-line react-hooks/set-state-in-effect
    if (!drag) setProgress(connected ? 1 : 0);
  }, [connected, drag]);

  const canDock = server !== null; // без выбранного сервера докать некуда

  const onDown = (e: React.PointerEvent<HTMLDivElement>) => {
    e.stopPropagation();
    if (busy) return;
    try {
      e.currentTarget.setPointerCapture(e.pointerId);
    } catch {
      /* no-op */
    }
    downY.current = e.clientY;
    moved.current = false;
    setDrag(true);
  };
  const onMove = (e: React.PointerEvent<HTMLDivElement>) => {
    if (!drag || !stageRef.current) return;
    if (Math.abs(e.clientY - downY.current) > 4) moved.current = true;
    const r = stageRef.current.getBoundingClientRect();
    const rel = (e.clientY - r.top) / r.height;
    setProgress(clamp((homeRel - rel) / (homeRel - slotRel), 0, 1));
  };
  const onUp = (e: React.PointerEvent<HTMLDivElement>) => {
    if (!drag) return;
    try {
      e.currentTarget.releasePointerCapture(e.pointerId);
    } catch {
      /* no-op */
    }
    setDrag(false);
    if (!moved.current) {
      // тап: если сервер не выбран — отправляем выбирать, иначе toggle
      if (!canDock && !connected) onOpenLocations();
      else onToggle();
      return;
    }
    const docked = progress > 0.72;
    if (docked && !canDock && !connected) {
      // дотащил до дока, но сервер не выбран — открываем список
      setProgress(0);
      onOpenLocations();
      return;
    }
    if (docked !== connected) onToggle();
    setProgress(docked ? 1 : 0);
  };

  const p = progress;
  const capRel = homeRel - p * (homeRel - slotRel);
  const seated = p > 0.97 && !drag;
  const near = p > 0.55;

  // морфинг: широкая pill → круглый disc
  const W = 320 - (320 - 80) * p; // 320 → 80
  const H = 66 + (80 - 66) * p; // 66 → 80
  // верх стрелки-подсказки над центром капсулы — до него же доходит пунктир
  const arrowOffset = H / 2 + ARROW_GAP;
  const pillOp = clamp(1 - p / 0.42, 0, 1);
  const discOp = clamp((p - 0.45) / 0.4, 0, 1);

  const trans = drag
    ? "none"
    : "top 0.5s cubic-bezier(.4,.05,.2,1), width 0.5s cubic-bezier(.4,.05,.2,1), height 0.5s cubic-bezier(.4,.05,.2,1), background 0.4s, color 0.4s, box-shadow 0.3s";

  const code = server ? countryCodeFromName(server.name) : "";
  const subLabel = busy
    ? connected
      ? "DISCONNECTING…"
      : "CONNECTING…"
    : !canDock
      ? "PICK A NODE →"
      : drag
        ? "KEEP DRAGGING ↑"
        : "DRAG TO DOCK";

  return (
    <div className="vrox-screen" style={{ position: "relative" }}>
      {/* Header — wordmark + статус */}
      <div
        style={{
          position: "relative",
          zIndex: 3,
          padding: "60px 24px 0",
          display: "flex",
          justifyContent: "space-between",
          alignItems: "center",
        }}
      >
        <VroxWordmark size={17} />
        <span className="tag">{connected ? "DOCKED" : "UNDOCKED"}</span>
      </div>

      {/* Полноэкранная сцена перетаскивания — слот ровно по центру экрана */}
      <div ref={stageRef} style={{ position: "absolute", inset: 0, zIndex: 1 }}>
        {/* направляющая (пунктир) — от слота до верха стрелки-подсказки
            (arrowOffset), а не до самой капсулы: отрезок под стрелкой
            визуально упирался в карточку сервера и выглядел лишним */}
        <div
          style={{
            position: "absolute",
            left: "50%",
            top: slotRel * 100 + "%",
            height: `calc(${(homeRel - slotRel) * 100}% - ${arrowOffset}px)`,
            width: 0,
            transform: "translateX(-50%)",
            borderLeft: "1.5px dashed var(--line-strong)",
            opacity: seated ? 0 : 0.55,
            transition: "opacity 0.4s",
          }}
        />
        {/* заполняющийся рельс прогресса */}
        <div
          style={{
            position: "absolute",
            left: "50%",
            top: capRel * 100 + "%",
            height: Math.max(0, homeRel - capRel) * 100 + "%",
            width: 2,
            transform: "translateX(-50%)",
            background: "var(--fg)",
            opacity: 0.2 + p * 0.5,
            transition: drag ? "none" : "all 0.5s",
          }}
        />

        {/* КРУГЛЫЙ СЛОТ */}
        <div
          onClick={() => {
            if (busy) return;
            if (!connected && canDock) onToggle();
            else if (!connected) onOpenLocations();
          }}
          style={{
            position: "absolute",
            left: "50%",
            top: slotRel * 100 + "%",
            transform: "translate(-50%, -50%)",
            width: 104,
            height: 104,
            borderRadius: "50%",
            cursor: connected ? "default" : "pointer",
            border: seated
              ? "1.5px solid var(--fg)"
              : near
                ? "1.5px solid var(--fg-muted)"
                : "1.5px dashed var(--line-strong)",
            background: seated ? "transparent" : "var(--bg-elev-2)",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            transition: "border-color 0.3s, background 0.4s",
          }}
        >
          {/* прицел-крестик */}
          {!seated && (
            <svg width="104" height="104" viewBox="0 0 104 104" style={{ position: "absolute", inset: 0, opacity: 0.4 }}>
              <line x1="52" y1="14" x2="52" y2="26" stroke="var(--fg-dim)" strokeWidth="1.2" />
              <line x1="52" y1="78" x2="52" y2="90" stroke="var(--fg-dim)" strokeWidth="1.2" />
              <line x1="14" y1="52" x2="26" y2="52" stroke="var(--fg-dim)" strokeWidth="1.2" />
              <line x1="78" y1="52" x2="90" y2="52" stroke="var(--fg-dim)" strokeWidth="1.2" />
            </svg>
          )}
          {/* пульс-кольца при стыковке */}
          {seated &&
            [0, 1, 2].map((i) => (
              <div
                key={i}
                style={{
                  position: "absolute",
                  width: 104,
                  height: 104,
                  borderRadius: "50%",
                  border: "1px solid var(--fg)",
                  opacity: 0,
                  animation: "vrox-ping 3s ease-out infinite",
                  animationDelay: i + "s",
                }}
              />
            ))}
        </div>

        {/* КАПСУЛА → ДИСК (перетаскиваемая) */}
        <div
          onPointerDown={onDown}
          onPointerMove={onMove}
          onPointerUp={onUp}
          onPointerCancel={onUp}
          style={{
            position: "absolute",
            left: "50%",
            top: capRel * 100 + "%",
            transform: "translate(-50%, -50%)",
            width: W,
            height: H,
            borderRadius: 999,
            overflow: "hidden",
            cursor: drag ? "grabbing" : "grab",
            touchAction: "none",
            background: seated ? "var(--fg)" : "var(--bg-elev-1)",
            color: seated ? "var(--bg)" : "var(--fg)",
            border: "1px solid " + (seated ? "transparent" : "var(--line-strong)"),
            boxShadow: drag
              ? "0 18px 40px rgba(0,0,0,0.35)"
              : seated
                ? "0 8px 26px rgba(0,0,0,0.3), inset 0 1px 0 rgba(255,255,255,0.25)"
                : "0 6px 18px rgba(0,0,0,0.18)",
            transition: trans,
            zIndex: 5,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
          }}
        >
          {/* содержимое pill */}
          <div
            style={{
              position: "absolute",
              left: 0,
              top: 0,
              height: "100%",
              width: 320,
              display: "flex",
              alignItems: "center",
              gap: 14,
              padding: "0 8px 0 18px",
              opacity: pillOp,
              whiteSpace: "nowrap",
              pointerEvents: "none",
            }}
          >
            <FlagDot code={code} size={34} />
            <div style={{ flex: 1, minWidth: 0 }}>
              <div
                style={{
                  fontSize: 15,
                  fontWeight: 500,
                  letterSpacing: "-0.01em",
                  overflow: "hidden",
                  textOverflow: "ellipsis",
                }}
              >
                {server ? server.name : "Select a node"}
              </div>
              <div className="mono" style={{ fontSize: 9, letterSpacing: "0.2em", marginTop: 2, color: "var(--fg-dim)" }}>
                {subLabel}
              </div>
            </div>
            <button
              onPointerDown={(e) => e.stopPropagation()}
              onClick={(e) => {
                e.stopPropagation();
                onOpenLocations();
              }}
              style={{
                width: 52,
                height: 52,
                borderRadius: 999,
                border: "none",
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
                background: "var(--bg-elev-3)",
                color: "var(--fg)",
                cursor: "pointer",
                pointerEvents: pillOp > 0.2 ? "auto" : "none",
              }}
            >
              <Ic.globe s={20} />
            </button>
          </div>
          {/* содержимое disc */}
          <div
            style={{
              position: "absolute",
              inset: 0,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              opacity: discOp,
              pointerEvents: "none",
            }}
          >
            {seated ? <Ic.lock s={26} /> : <FlagDot code={code} size={42} />}
          </div>
        </div>

        {/* подсказка-стрелка над капсулой */}
        {!near && !connected && !busy && (
          <div
            style={{
              position: "absolute",
              left: "50%",
              top: capRel * 100 + "%",
              transform: "translate(-50%, 0)",
              marginTop: -arrowOffset,
              color: "var(--fg-dim)",
              animation: "vrox-hint 1.6s ease-in-out infinite",
              pointerEvents: "none",
            }}
          >
            <svg width="22" height="30" viewBox="0 0 22 30" fill="none">
              <path d="M11 28V5M4 12l7-7 7 7" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" />
            </svg>
          </div>
        )}

        {/* подпись снизу */}
        <div
          style={{
            position: "absolute",
            left: 0,
            right: 0,
            bottom: 40,
            textAlign: "center",
            zIndex: 6,
            fontFamily: "var(--font-display, var(--font-ui))",
          }}
        >
          <button
            onClick={onOpenLocations}
            className="mono"
            style={{
              background: "transparent",
              border: "none",
              cursor: "pointer",
              color: "var(--fg-muted)",
              fontSize: 10,
              letterSpacing: "0.3em",
            }}
          >
            CHANGE REGION
          </button>
        </div>
      </div>
    </div>
  );
}
