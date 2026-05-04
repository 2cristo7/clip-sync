export function BroadcastPage() {
  return (
    <div className="p-6">
      <h1 className="text-lg font-semibold mb-1">Broadcast</h1>
      <p className="text-sm text-[var(--color-text-secondary)] mb-6">
        Send clipboard content to all or selected devices.
      </p>
      <div className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-secondary)] p-8 text-center text-sm text-[var(--color-text-secondary)]">
        Broadcast requires an active server connection.
      </div>
    </div>
  );
}
