import type { useWireGuardForm } from "@/hooks";

interface WireGuardFormProps {
  form: ReturnType<typeof useWireGuardForm>;
  onDone: () => void;
}

const CONF_PLACEHOLDER = "[Interface]\nPrivateKey = …\nAddress = 10.8.0.2/24\n\n[Peer]\nPublicKey = …\nEndpoint = vpn.example.com:51820";

// Импорт WireGuard/AmneziaWG: вставить текст .conf, выбрать файл или
// отсканировать QR (wg-easy показывает его для каждого клиента).
export function WireGuardForm({ form, onDone }: WireGuardFormProps) {
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
      <div className="add-row">
        <button type="button" className="btn-secondary" onClick={form.pickFile}>
          Choose file
        </button>
        <button type="button" className="btn-secondary" onClick={form.scanQr}>
          Scan QR
        </button>
      </div>
      <textarea
        className="conf-input"
        value={form.conf}
        onChange={(event) => form.setConf(event.currentTarget.value)}
        placeholder={CONF_PLACEHOLDER}
        aria-label="WireGuard config"
        spellCheck={false}
        autoCapitalize="off"
        autoCorrect="off"
      />
      <input
        className="text-input"
        value={form.name}
        onChange={(event) => form.setName(event.currentTarget.value)}
        placeholder="Name"
        aria-label="Name"
      />
      {form.error && <div className="banner error">{form.error}</div>}
      <button type="submit" className="btn-primary">
        Add WireGuard
      </button>
    </form>
  );
}
