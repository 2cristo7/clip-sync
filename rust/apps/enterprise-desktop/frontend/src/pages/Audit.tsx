import { useState, useEffect, useCallback, useMemo, useRef } from "react";
import type { AuditEntry, AuditEventType } from "../types/audit";
import {
  ALL_EVENT_TYPES,
  AUDIT_EVENT_LABELS,
} from "../types/audit";
import { AuditTable } from "../components/AuditTable";

/* ------------------------------------------------------------------ */
/*  Mock data generator                                                */
/* ------------------------------------------------------------------ */

const MOCK_DEVICES = [
  { id: "dev-001", name: "MacBook Pro" },
  { id: "dev-002", name: "Pixel 8" },
  { id: "dev-003", name: "iPad Air" },
  { id: "dev-004", name: "Galaxy S24" },
  { id: "dev-005", name: "iMac Studio" },
];

const MOCK_ACTORS = ["admin@corp.io", "user1@corp.io", "system", "user2@corp.io"];

const MOCK_DETAILS: Record<AuditEventType, string[]> = {
  device_paired: ["TOFU pairing accepted", "Manual approval by admin"],
  device_revoked: ["Revoked by admin", "Auto-revoked: inactive 30d"],
  clipboard_pushed: ["Text (142 bytes)", "Image (1.2 MB)", "Text (38 bytes)"],
  clipboard_delivered: ["Delivered in 23ms", "Delivered in 112ms"],
  broadcast_sent: ["Deploy notice to all devices", "Maintenance window alert"],
  broadcast_delivered: ["Received by 4/5 devices", "Received by 5/5 devices"],
  policy_changed: ["send_only -> full_access", "full_access -> disabled"],
  connection_opened: ["WebSocket established", "Reconnected after timeout"],
  connection_closed: ["Client disconnected", "Idle timeout (300s)"],
};

function generateMockEntries(count: number): AuditEntry[] {
  const entries: AuditEntry[] = [];
  const now = Date.now();
  for (let i = 0; i < count; i++) {
    const eventType = ALL_EVENT_TYPES[i % ALL_EVENT_TYPES.length];
    const device = MOCK_DEVICES[i % MOCK_DEVICES.length];
    const details = MOCK_DETAILS[eventType];
    entries.push({
      id: `audit-${String(i).padStart(6, "0")}`,
      timestamp: new Date(now - i * 8_000).toISOString(),
      event_type: eventType,
      device_id: device.id,
      device_name: device.name,
      detail: details[i % details.length],
      actor: MOCK_ACTORS[i % MOCK_ACTORS.length],
    });
  }
  return entries;
}

/* ------------------------------------------------------------------ */
/*  CSV export                                                         */
/* ------------------------------------------------------------------ */

function escapeCsvField(value: string): string {
  if (value.includes(",") || value.includes('"') || value.includes("\n")) {
    return `"${value.replace(/"/g, '""')}"`;
  }
  return value;
}

function entriesToCsv(entries: AuditEntry[]): string {
  const header = "id,timestamp,event_type,device_id,device_name,actor,detail";
  const rows = entries.map(
    (e) =>
      [
        e.id,
        e.timestamp,
        e.event_type,
        e.device_id,
        escapeCsvField(e.device_name),
        escapeCsvField(e.actor),
        escapeCsvField(e.detail),
      ].join(","),
  );
  return [header, ...rows].join("\n");
}

function downloadCsv(csv: string, filename: string) {
  const blob = new Blob([csv], { type: "text/csv;charset=utf-8;" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  a.click();
  URL.revokeObjectURL(url);
}

/* ------------------------------------------------------------------ */
/*  Date helpers                                                       */
/* ------------------------------------------------------------------ */

function toLocalDateInput(date: Date): string {
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}

/* ------------------------------------------------------------------ */
/*  Page component                                                     */
/* ------------------------------------------------------------------ */

const ALL_MOCK = generateMockEntries(12_000);

export function AuditPage() {
  /* Filters */
  const [dateFrom, setDateFrom] = useState(() => {
    const d = new Date();
    d.setDate(d.getDate() - 7);
    return toLocalDateInput(d);
  });
  const [dateTo, setDateTo] = useState(() => toLocalDateInput(new Date()));
  const [deviceFilter, setDeviceFilter] = useState<string>("all");
  const [eventTypeFilter, setEventTypeFilter] = useState<string>("all");

  /* Real-time tail */
  const [tailEnabled, setTailEnabled] = useState(false);
  const [tailEntries, setTailEntries] = useState<AuditEntry[]>([]);
  const tailCounter = useRef(0);

  /* Simulate incoming audit events when tail is on */
  useEffect(() => {
    if (!tailEnabled) return;
    const interval = setInterval(() => {
      tailCounter.current += 1;
      const idx = tailCounter.current;
      const eventType = ALL_EVENT_TYPES[idx % ALL_EVENT_TYPES.length];
      const device = MOCK_DEVICES[idx % MOCK_DEVICES.length];
      const details = MOCK_DETAILS[eventType];
      const entry: AuditEntry = {
        id: `tail-${Date.now()}-${idx}`,
        timestamp: new Date().toISOString(),
        event_type: eventType,
        device_id: device.id,
        device_name: device.name,
        detail: details[idx % details.length],
        actor: MOCK_ACTORS[idx % MOCK_ACTORS.length],
      };
      setTailEntries((prev) => [entry, ...prev].slice(0, 200));
    }, 1500);
    return () => clearInterval(interval);
  }, [tailEnabled]);

  /* Clear tail entries when toggling off */
  useEffect(() => {
    if (!tailEnabled) {
      setTailEntries([]);
    }
  }, [tailEnabled]);

  /* Apply filters */
  const filtered = useMemo(() => {
    const fromTs = dateFrom ? new Date(dateFrom).getTime() : 0;
    const toTs = dateTo
      ? new Date(dateTo).getTime() + 86_400_000
      : Number.MAX_SAFE_INTEGER;

    return ALL_MOCK.filter((e) => {
      const ts = new Date(e.timestamp).getTime();
      if (ts < fromTs || ts > toTs) return false;
      if (deviceFilter !== "all" && e.device_id !== deviceFilter) return false;
      if (eventTypeFilter !== "all" && e.event_type !== eventTypeFilter)
        return false;
      return true;
    });
  }, [dateFrom, dateTo, deviceFilter, eventTypeFilter]);

  /* Merge tail entries at top */
  const displayEntries = useMemo(() => {
    if (!tailEnabled || tailEntries.length === 0) return filtered;
    return [...tailEntries, ...filtered];
  }, [tailEnabled, tailEntries, filtered]);

  /* Unique devices for the filter dropdown */
  const uniqueDevices = useMemo(() => {
    const map = new Map<string, string>();
    for (const e of ALL_MOCK) {
      if (!map.has(e.device_id)) map.set(e.device_id, e.device_name);
    }
    return Array.from(map.entries());
  }, []);

  const handleExport = useCallback(() => {
    const csv = entriesToCsv(displayEntries);
    const ts = new Date().toISOString().replace(/[:.]/g, "-");
    downloadCsv(csv, `clipsync-audit-${ts}.csv`);
  }, [displayEntries]);

  return (
    <div className="p-6 flex flex-col gap-4 h-full">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-lg font-semibold mb-1">Audit Log</h1>
          <p className="text-sm text-[var(--color-text-secondary)]">
            View clipboard sync events and security actions.
          </p>
        </div>
        <div className="flex items-center gap-2">
          <span className="text-xs text-[var(--color-text-secondary)]">
            {displayEntries.length.toLocaleString()} events
          </span>
        </div>
      </div>

      {/* Toolbar */}
      <div className="flex flex-wrap items-end gap-3">
        {/* Date From */}
        <label className="flex flex-col gap-1">
          <span className="text-[11px] font-medium uppercase tracking-wider text-[var(--color-text-secondary)]">
            From
          </span>
          <input
            type="date"
            value={dateFrom}
            onChange={(e) => setDateFrom(e.target.value)}
            className="h-8 px-2 text-xs rounded-md border border-[var(--color-border)] bg-[var(--color-bg-primary)] text-[var(--color-text-primary)] focus:outline-none focus:ring-1 focus:ring-brand-500"
          />
        </label>

        {/* Date To */}
        <label className="flex flex-col gap-1">
          <span className="text-[11px] font-medium uppercase tracking-wider text-[var(--color-text-secondary)]">
            To
          </span>
          <input
            type="date"
            value={dateTo}
            onChange={(e) => setDateTo(e.target.value)}
            className="h-8 px-2 text-xs rounded-md border border-[var(--color-border)] bg-[var(--color-bg-primary)] text-[var(--color-text-primary)] focus:outline-none focus:ring-1 focus:ring-brand-500"
          />
        </label>

        {/* Device filter */}
        <label className="flex flex-col gap-1">
          <span className="text-[11px] font-medium uppercase tracking-wider text-[var(--color-text-secondary)]">
            Device
          </span>
          <select
            value={deviceFilter}
            onChange={(e) => setDeviceFilter(e.target.value)}
            className="h-8 px-2 text-xs rounded-md border border-[var(--color-border)] bg-[var(--color-bg-primary)] text-[var(--color-text-primary)] focus:outline-none focus:ring-1 focus:ring-brand-500"
          >
            <option value="all">All Devices</option>
            {uniqueDevices.map(([id, name]) => (
              <option key={id} value={id}>
                {name}
              </option>
            ))}
          </select>
        </label>

        {/* Event type filter */}
        <label className="flex flex-col gap-1">
          <span className="text-[11px] font-medium uppercase tracking-wider text-[var(--color-text-secondary)]">
            Event Type
          </span>
          <select
            value={eventTypeFilter}
            onChange={(e) => setEventTypeFilter(e.target.value)}
            className="h-8 px-2 text-xs rounded-md border border-[var(--color-border)] bg-[var(--color-bg-primary)] text-[var(--color-text-primary)] focus:outline-none focus:ring-1 focus:ring-brand-500"
          >
            <option value="all">All Events</option>
            {ALL_EVENT_TYPES.map((t) => (
              <option key={t} value={t}>
                {AUDIT_EVENT_LABELS[t]}
              </option>
            ))}
          </select>
        </label>

        {/* Spacer */}
        <div className="flex-1" />

        {/* Real-time tail toggle */}
        <button
          type="button"
          onClick={() => setTailEnabled((v) => !v)}
          className={`h-8 px-3 text-xs font-medium rounded-md border transition-colors ${
            tailEnabled
              ? "border-green-500 bg-green-500/10 text-green-700 dark:text-green-400"
              : "border-[var(--color-border)] bg-[var(--color-bg-primary)] text-[var(--color-text-secondary)] hover:bg-[var(--color-bg-secondary)]"
          }`}
        >
          {tailEnabled ? "Live" : "Tail"}
          {tailEnabled && (
            <span className="inline-block w-1.5 h-1.5 rounded-full bg-green-500 ml-1.5 animate-pulse" />
          )}
        </button>

        {/* CSV Export */}
        <button
          type="button"
          onClick={handleExport}
          disabled={displayEntries.length === 0}
          className="h-8 px-3 text-xs font-medium rounded-md border border-[var(--color-border)] bg-[var(--color-bg-primary)] text-[var(--color-text-secondary)] hover:bg-[var(--color-bg-secondary)] disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
        >
          Export CSV
        </button>
      </div>

      {/* Table */}
      <div className="flex-1 min-h-0">
        <AuditTable entries={displayEntries} />
      </div>
    </div>
  );
}
