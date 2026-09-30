import type { useServerForm } from "@/hooks";

interface ServerFormProps {
  form: ReturnType<typeof useServerForm>;
  // файл и QR — только iOS (на Linux плагинов нет, там только вставка)
  canPickFileOrQr: boolean;
  onDone: () => void;
}

const TEXT_PLACEHOLDER = "hysteria2://…\n\nor a WireGuard config:\n[Interface]\nPrivateKey = …\n[Peer]\nEndpoint = vpn.example.com:51820";

// Вкладка «Server»: один сервер — ссылка hysteria2:// или .conf
// WireGuard/AmneziaWG. Протокол определяется автоматически.
export function ServerForm({ form, canPickFileOrQr, onDone }: ServerFormProps) {
  async function onSubmit() {
    if (await form.confirm()) onDone();
  }

  return (
    <form
      className="add-form"
      onSubmit={(event) => {
        event.preventDefault();
        onSubmit();
      }}
    >
      {canPickFileOrQr && (
        <div className="add-row">
          <button type="button" className="btn-secondary" onClick={form.pickFile}>
            Choose file
          </button>
          <button type="button" className="btn-secondary" onClick={form.scanQr}>
            Scan QR
          </button>
        </div>
      )}
      <textarea
        className="conf-input"
        value={form.text}
        onChange={(event) => form.setText(event.currentTarget.value)}
        placeholder={TEXT_PLACEHOLDER}
        aria-label="Server link or config"
        spellCheck={false}
        autoCapitalize="off"
        autoCorrect="off"
      />
      <input
        className="text-input"
        value={form.name}
        onChange={(event) => form.setName(event.currentTarget.value)}
        placeholder="Name (optional)"
        aria-label="Name"
      />
      {form.error && <div className="banner error">{form.error}</div>}
      <button type="submit" className="btn-primary">
        Add server
      </button>
    </form>
  );
}
