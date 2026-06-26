interface BottomActionProps {
  hasSubscriptions: boolean;
  onAdd: () => void;
  onPaste: () => void;
  connected: boolean;
  busy: boolean;
  serverName: string | null;
  selectedServerName: string | undefined;
  onToggle: () => void;
}

export function BottomAction({
  hasSubscriptions,
  onAdd,
  onPaste,
  connected,
  busy,
  serverName,
  selectedServerName,
  onToggle,
}: BottomActionProps) {
  if (!hasSubscriptions) {
    return (
      <div className="bottom-action">
        <div className="bottom-action-row">
          <button className="connect-button suggested" onClick={onAdd}>
            Добавить
          </button>
          <button className="connect-button outline" onClick={onPaste}>
            Вставить из буфера
          </button>
        </div>
      </div>
    );
  }

  const label = busy
    ? connected
      ? "Отключение…"
      : "Подключение…"
    : connected
      ? `Отключиться (${serverName})`
      : selectedServerName
        ? `Подключиться (${selectedServerName})`
        : "Выбери сервер";

  return (
    <div className="bottom-action">
      <div className="bottom-action-row">
        <button
          className={`connect-button main ${connected ? "destructive" : "suggested"}`}
          onClick={onToggle}
          disabled={busy || (!connected && !selectedServerName)}
        >
          {label}
        </button>
        <button
          className="connect-button add-small"
          title="Добавить подписку"
          aria-label="Добавить подписку"
          onClick={onAdd}
        >
          +
        </button>
      </div>
    </div>
  );
}
