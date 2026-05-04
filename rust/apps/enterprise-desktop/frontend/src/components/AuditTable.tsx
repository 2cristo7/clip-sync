import { useRef, useMemo } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import type { AuditEntry } from "../types/audit";
import { AUDIT_EVENT_LABELS } from "../types/audit";

interface AuditTableProps {
  entries: AuditEntry[];
}

const ROW_HEIGHT = 36;

const EVENT_BADGE_COLORS: Record<string, string> = {
  device_paired:
    "bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-400",
  device_revoked:
    "bg-red-100 text-red-800 dark:bg-red-900/30 dark:text-red-400",
  clipboard_pushed:
    "bg-blue-100 text-blue-800 dark:bg-blue-900/30 dark:text-blue-400",
  clipboard_delivered:
    "bg-blue-100 text-blue-700 dark:bg-blue-900/20 dark:text-blue-300",
  broadcast_sent:
    "bg-purple-100 text-purple-800 dark:bg-purple-900/30 dark:text-purple-400",
  broadcast_delivered:
    "bg-purple-100 text-purple-700 dark:bg-purple-900/20 dark:text-purple-300",
  policy_changed:
    "bg-amber-100 text-amber-800 dark:bg-amber-900/30 dark:text-amber-400",
  connection_opened:
    "bg-cyan-100 text-cyan-800 dark:bg-cyan-900/30 dark:text-cyan-400",
  connection_closed:
    "bg-gray-100 text-gray-700 dark:bg-gray-800/40 dark:text-gray-400",
};

function formatTimestamp(iso: string): string {
  const d = new Date(iso);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
}

export function AuditTable({ entries }: AuditTableProps) {
  const parentRef = useRef<HTMLDivElement>(null);

  const rowVirtualizer = useVirtualizer({
    count: entries.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 20,
  });

  const virtualItems = rowVirtualizer.getVirtualItems();
  const totalSize = rowVirtualizer.getTotalSize();

  const paddingTop = useMemo(
    () => (virtualItems.length > 0 ? virtualItems[0].start : 0),
    [virtualItems],
  );
  const paddingBottom = useMemo(
    () =>
      virtualItems.length > 0
        ? totalSize - virtualItems[virtualItems.length - 1].end
        : 0,
    [virtualItems, totalSize],
  );

  if (entries.length === 0) {
    return (
      <div className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-secondary)] p-8 text-center text-sm text-[var(--color-text-secondary)]">
        No audit events match the current filters.
      </div>
    );
  }

  return (
    <div className="rounded-lg border border-[var(--color-border)] overflow-hidden">
      {/* Header */}
      <div className="grid grid-cols-[160px_140px_140px_140px_1fr] bg-[var(--color-bg-secondary)] border-b border-[var(--color-border)] text-[11px] font-semibold uppercase tracking-wider text-[var(--color-text-secondary)]">
        <div className="px-3 py-2">Timestamp</div>
        <div className="px-3 py-2">Event</div>
        <div className="px-3 py-2">Device</div>
        <div className="px-3 py-2">Actor</div>
        <div className="px-3 py-2">Detail</div>
      </div>

      {/* Virtualized body */}
      <div ref={parentRef} className="overflow-auto max-h-[calc(100vh-280px)]">
        <div style={{ height: `${totalSize}px`, position: "relative" }}>
          {paddingTop > 0 && <div style={{ height: `${paddingTop}px` }} />}
          {virtualItems.map((virtualRow) => {
            const entry = entries[virtualRow.index];
            const badgeColor =
              EVENT_BADGE_COLORS[entry.event_type] ??
              "bg-gray-100 text-gray-700 dark:bg-gray-800 dark:text-gray-400";

            return (
              <div
                key={entry.id}
                className="grid grid-cols-[160px_140px_140px_140px_1fr] border-b border-[var(--color-border)] hover:bg-[var(--color-bg-secondary)] transition-colors text-sm"
                style={{ height: `${ROW_HEIGHT}px` }}
              >
                <div className="px-3 flex items-center text-xs font-mono text-[var(--color-text-secondary)] truncate">
                  {formatTimestamp(entry.timestamp)}
                </div>
                <div className="px-3 flex items-center">
                  <span
                    className={`inline-block px-1.5 py-0.5 rounded text-[10px] font-medium leading-tight ${badgeColor}`}
                  >
                    {AUDIT_EVENT_LABELS[entry.event_type] ?? entry.event_type}
                  </span>
                </div>
                <div className="px-3 flex items-center text-xs truncate">
                  {entry.device_name}
                </div>
                <div className="px-3 flex items-center text-xs text-[var(--color-text-secondary)] truncate">
                  {entry.actor}
                </div>
                <div className="px-3 flex items-center text-xs text-[var(--color-text-secondary)] truncate">
                  {entry.detail}
                </div>
              </div>
            );
          })}
          {paddingBottom > 0 && <div style={{ height: `${paddingBottom}px` }} />}
        </div>
      </div>
    </div>
  );
}
