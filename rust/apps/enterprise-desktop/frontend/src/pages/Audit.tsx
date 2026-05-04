export function AuditPage() {
  return (
    <div className="p-6">
      <h1 className="text-lg font-semibold mb-1">Audit Log</h1>
      <p className="text-sm text-[var(--color-text-secondary)] mb-6">
        View clipboard sync events and security actions.
      </p>
      <div className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-secondary)] p-8 text-center text-sm text-[var(--color-text-secondary)]">
        No audit events recorded. Events will appear once the server is connected.
      </div>
    </div>
  );
}
