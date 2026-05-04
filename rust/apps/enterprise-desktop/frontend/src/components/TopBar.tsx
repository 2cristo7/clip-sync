import { useTheme } from "../hooks/useTheme";
import type { ConnectionStatus } from "../hooks/useConnectionStatus";

interface TopBarProps {
  connectionStatus: ConnectionStatus;
}

const STATUS_CONFIG: Record<ConnectionStatus, { label: string; dotClass: string }> = {
  connected: { label: "Connected", dotClass: "bg-emerald-500" },
  disconnected: { label: "Disconnected", dotClass: "bg-gray-400" },
  connecting: { label: "Connecting", dotClass: "bg-amber-400 animate-pulse" },
};

export function TopBar({ connectionStatus }: TopBarProps) {
  const { theme, setTheme } = useTheme();
  const status = STATUS_CONFIG[connectionStatus];

  return (
    <header className="flex items-center justify-between h-12 px-5 border-b border-[var(--color-border)] bg-[var(--color-bg-primary)]">
      <div className="flex items-center gap-2">
        <span className="text-sm font-medium text-[var(--color-text-secondary)]">
          Dashboard
        </span>
      </div>
      <div className="flex items-center gap-4">
        <div className="flex items-center gap-2 text-xs text-[var(--color-text-secondary)]">
          <span className={`w-2 h-2 rounded-full ${status.dotClass}`} />
          <span>{status.label}</span>
        </div>
        <div className="h-4 w-px bg-[var(--color-border)]" />
        <select
          value={theme}
          onChange={(e) => setTheme(e.target.value as "light" | "dark" | "system")}
          className="text-xs bg-transparent text-[var(--color-text-secondary)] border border-[var(--color-border)] rounded px-1.5 py-0.5 cursor-pointer focus:outline-none focus:ring-1 focus:ring-brand-500"
        >
          <option value="system">System</option>
          <option value="light">Light</option>
          <option value="dark">Dark</option>
        </select>
      </div>
    </header>
  );
}
