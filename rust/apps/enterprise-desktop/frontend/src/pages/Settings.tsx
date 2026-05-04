export function SettingsPage() {
  return (
    <div className="p-6">
      <h1 className="text-lg font-semibold mb-1">Settings</h1>
      <p className="text-sm text-[var(--color-text-secondary)] mb-6">
        Configure server connection and admin credentials.
      </p>
      <div className="space-y-4">
        <div className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-secondary)] p-4">
          <h2 className="text-sm font-medium mb-2">Server Connection</h2>
          <div className="grid gap-3">
            <label className="block">
              <span className="text-xs text-[var(--color-text-secondary)]">Server URL</span>
              <input
                type="text"
                placeholder="ws://localhost:9900"
                disabled
                className="mt-1 block w-full rounded-md border border-[var(--color-border)] bg-[var(--color-bg-primary)] px-3 py-1.5 text-sm disabled:opacity-50"
              />
            </label>
            <label className="block">
              <span className="text-xs text-[var(--color-text-secondary)]">Admin Token</span>
              <input
                type="password"
                placeholder="Set via CLIPSYNC_ADMIN_TOKEN"
                disabled
                className="mt-1 block w-full rounded-md border border-[var(--color-border)] bg-[var(--color-bg-primary)] px-3 py-1.5 text-sm disabled:opacity-50"
              />
            </label>
          </div>
        </div>
      </div>
    </div>
  );
}
