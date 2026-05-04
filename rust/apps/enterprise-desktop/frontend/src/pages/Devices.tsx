export function DevicesPage() {
  return (
    <div className="p-6">
      <h1 className="text-lg font-semibold mb-1">Devices</h1>
      <p className="text-sm text-[var(--color-text-secondary)] mb-6">
        Manage registered devices and their policies.
      </p>
      <div className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-secondary)] p-8 text-center text-sm text-[var(--color-text-secondary)]">
        No devices registered yet. Connect the enterprise server to see devices.
      </div>
    </div>
  );
}
