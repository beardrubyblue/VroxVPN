import type { useAddSubscription } from "@/hooks";

interface SubscriptionFormProps {
  form: ReturnType<typeof useAddSubscription>;
  onDone: () => void;
}

export function SubscriptionForm({ form, onDone }: SubscriptionFormProps) {
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
      <input
        className="text-input"
        value={form.url}
        onChange={(event) => form.setUrl(event.currentTarget.value)}
        placeholder="https://sub.example.com/…"
        aria-label="Subscription URL"
        autoFocus
      />
      {form.error && <div className="banner error">{form.error}</div>}
      <button type="submit" className="btn-primary">
        Add subscription
      </button>
    </form>
  );
}
