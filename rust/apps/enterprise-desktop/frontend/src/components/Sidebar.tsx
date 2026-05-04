import { NavLink } from "react-router-dom";

const NAV_ITEMS = [
  { to: "/devices", label: "Devices", icon: DevicesIcon },
  { to: "/audit", label: "Audit", icon: AuditIcon },
  { to: "/broadcast", label: "Broadcast", icon: BroadcastIcon },
  { to: "/settings", label: "Settings", icon: SettingsIcon },
] as const;

export function Sidebar() {
  return (
    <aside className="flex flex-col w-56 shrink-0 border-r border-[var(--color-border)] bg-[var(--color-bg-sidebar)] h-screen">
      <div className="flex items-center gap-2 px-5 py-4 border-b border-[var(--color-border)]">
        <div className="w-6 h-6 rounded bg-brand-500" />
        <span className="text-sm font-semibold tracking-tight">ClipSync</span>
        <span className="text-[10px] font-medium text-[var(--color-text-secondary)] uppercase tracking-wider ml-auto">
          Enterprise
        </span>
      </div>
      <nav className="flex-1 py-2 px-2 space-y-0.5">
        {NAV_ITEMS.map(({ to, label, icon: Icon }) => (
          <NavLink
            key={to}
            to={to}
            className={({ isActive }) =>
              `flex items-center gap-2.5 px-3 py-2 rounded-md text-sm transition-colors ${
                isActive
                  ? "bg-brand-500/10 text-brand-600 dark:text-brand-400 font-medium"
                  : "text-[var(--color-text-secondary)] hover:bg-gray-200/60 dark:hover:bg-gray-800/60 hover:text-[var(--color-text-primary)]"
              }`
            }
          >
            <Icon className="w-4 h-4 shrink-0" />
            {label}
          </NavLink>
        ))}
      </nav>
      <div className="px-4 py-3 border-t border-[var(--color-border)] text-[11px] text-[var(--color-text-secondary)]">
        v0.1.0
      </div>
    </aside>
  );
}

function DevicesIcon({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
      <rect x="1" y="3" width="10" height="7" rx="1" />
      <path d="M4 13h4M6 10v3" />
      <rect x="12" y="5" width="3" height="6" rx="0.5" />
    </svg>
  );
}

function AuditIcon({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
      <path d="M4 1v14M12 1v14M1 4h14M1 8h14M1 12h14" />
    </svg>
  );
}

function BroadcastIcon({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
      <circle cx="8" cy="8" r="2" />
      <path d="M4.5 4.5a5 5 0 0 1 7 0M2.5 2.5a8 8 0 0 1 11 0M4.5 11.5a5 5 0 0 0 7 0M2.5 13.5a8 8 0 0 0 11 0" />
    </svg>
  );
}

function SettingsIcon({ className }: { className?: string }) {
  return (
    <svg className={className} viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
      <circle cx="8" cy="8" r="2.5" />
      <path d="M8 1v2M8 13v2M1 8h2M13 8h2M2.9 2.9l1.4 1.4M11.7 11.7l1.4 1.4M13.1 2.9l-1.4 1.4M4.3 11.7l-1.4 1.4" />
    </svg>
  );
}
