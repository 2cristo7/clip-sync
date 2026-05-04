import { useCallback, useEffect, useMemo, useState } from "react";
import type { Device, PolicyMode } from "../types/device";
import { POLICY_LABELS } from "../types/device";
import {
  fetchDevices,
  subscribeDeviceStatus,
  changePolicy,
  revokeDevice,
  kickSession,
} from "../api/client";
import { DeviceDetailDrawer } from "../components/DeviceDetailDrawer";

// ---------------------------------------------------------------
// Sorting
// ---------------------------------------------------------------

type SortKey =
  | "name"
  | "role"
  | "status"
  | "policy"
  | "last_seen"
  | "paired_at";
type SortDir = "asc" | "desc";

function compare(a: Device, b: Device, key: SortKey): number {
  switch (key) {
    case "name":
      return a.name.localeCompare(b.name);
    case "role":
      return a.role.localeCompare(b.role);
    case "status": {
      const rank = (d: Device) => (d.status === "online" ? 0 : 1);
      return rank(a) - rank(b);
    }
    case "policy":
      return a.policy.localeCompare(b.policy);
    case "last_seen":
      return (
        new Date(a.last_seen).getTime() - new Date(b.last_seen).getTime()
      );
    case "paired_at":
      return (
        new Date(a.paired_at).getTime() - new Date(b.paired_at).getTime()
      );
  }
}

// ---------------------------------------------------------------
// Column definitions
// ---------------------------------------------------------------

interface Column {
  key: SortKey;
  label: string;
}

const COLUMNS: Column[] = [
  { key: "name", label: "Name" },
  { key: "role", label: "Role" },
  { key: "status", label: "Status" },
  { key: "policy", label: "Policy" },
  { key: "last_seen", label: "Last Seen" },
  { key: "paired_at", label: "Paired" },
];

// ---------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------

function fmtRelative(iso: string): string {
  const diff = Date.now() - new Date(iso).getTime();
  if (diff < 60_000) return "just now";
  if (diff < 3_600_000) return `${Math.floor(diff / 60_000)}m ago`;
  if (diff < 86_400_000) return `${Math.floor(diff / 3_600_000)}h ago`;
  return new Date(iso).toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
  });
}

function fmtDate(iso: string): string {
  return new Date(iso).toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
    year: "numeric",
  });
}

// ---------------------------------------------------------------
// Page component
// ---------------------------------------------------------------

export function DevicesPage() {
  const [devices, setDevices] = useState<Device[]>([]);
  const [search, setSearch] = useState("");
  const [sortKey, setSortKey] = useState<SortKey>("name");
  const [sortDir, setSortDir] = useState<SortDir>("asc");
  const [selectedDevice, setSelectedDevice] = useState<Device | null>(null);
  const [confirmRevoke, setConfirmRevoke] = useState<string | null>(null);
  const [actionPending, setActionPending] = useState<string | null>(null);

  // ---- Fetch devices ----
  const loadDevices = useCallback(() => {
    fetchDevices().then(setDevices);
  }, []);

  useEffect(() => {
    loadDevices();
  }, [loadDevices]);

  // ---- WS subscription for real-time status ----
  useEffect(() => {
    const unsub = subscribeDeviceStatus((update) => {
      setDevices((prev) =>
        prev.map((d) =>
          d.id === update.device_id
            ? {
                ...d,
                status: update.status,
                latency_ms: update.latency_ms,
                last_seen: new Date().toISOString(),
              }
            : d,
        ),
      );
    });
    return unsub;
  }, []);

  // ---- Sorting ----
  function handleSort(key: SortKey) {
    if (sortKey === key) {
      setSortDir((d) => (d === "asc" ? "desc" : "asc"));
    } else {
      setSortKey(key);
      setSortDir("asc");
    }
  }

  // ---- Filtered + sorted list ----
  const visible = useMemo(() => {
    let list = devices;
    if (search.trim()) {
      const q = search.toLowerCase();
      list = list.filter(
        (d) =>
          d.name.toLowerCase().includes(q) || d.id.toLowerCase().includes(q),
      );
    }
    const sorted = [...list].sort((a, b) => compare(a, b, sortKey));
    return sortDir === "desc" ? sorted.reverse() : sorted;
  }, [devices, search, sortKey, sortDir]);

  // ---- Row actions ----
  async function handleChangePolicy(deviceId: string, policy: PolicyMode) {
    setActionPending(deviceId);
    await changePolicy(deviceId, policy);
    setDevices((prev) =>
      prev.map((d) => (d.id === deviceId ? { ...d, policy } : d)),
    );
    setActionPending(null);
  }

  async function handleRevoke(deviceId: string) {
    setActionPending(deviceId);
    await revokeDevice(deviceId);
    setDevices((prev) => prev.filter((d) => d.id !== deviceId));
    setConfirmRevoke(null);
    setActionPending(null);
  }

  async function handleKick(deviceId: string) {
    setActionPending(deviceId);
    await kickSession(deviceId);
    setActionPending(null);
  }

  return (
    <div className="p-6 h-full flex flex-col">
      {/* Header */}
      <div className="flex items-center justify-between mb-4">
        <div>
          <h1 className="text-lg font-semibold mb-0.5">Devices</h1>
          <p className="text-sm text-[var(--color-text-secondary)]">
            {devices.length} registered &middot;{" "}
            {devices.filter((d) => d.status === "online").length} online
          </p>
        </div>
        {/* Search */}
        <div className="relative">
          <svg
            className="absolute left-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-[var(--color-text-secondary)]"
            viewBox="0 0 16 16"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
          >
            <circle cx="6.5" cy="6.5" r="5" />
            <path d="M10.5 10.5L15 15" />
          </svg>
          <input
            type="text"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="Search by name or id..."
            className="pl-8 pr-3 py-1.5 w-56 text-xs rounded-md border border-[var(--color-border)] bg-[var(--color-bg-secondary)] text-[var(--color-text-primary)] placeholder:text-[var(--color-text-secondary)] focus:outline-none focus:ring-1 focus:ring-brand-500"
          />
        </div>
      </div>

      {/* Table */}
      <div className="flex-1 overflow-auto rounded-lg border border-[var(--color-border)]">
        <table className="w-full text-sm border-collapse">
          <thead>
            <tr className="bg-[var(--color-bg-secondary)] sticky top-0 z-10">
              {COLUMNS.map((col) => (
                <th
                  key={col.key}
                  onClick={() => handleSort(col.key)}
                  className="px-4 py-2.5 text-left text-xs font-medium text-[var(--color-text-secondary)] uppercase tracking-wider cursor-pointer select-none hover:text-[var(--color-text-primary)] transition-colors whitespace-nowrap"
                >
                  <span className="inline-flex items-center gap-1">
                    {col.label}
                    {sortKey === col.key && <SortArrow dir={sortDir} />}
                  </span>
                </th>
              ))}
              <th className="px-4 py-2.5 text-right text-xs font-medium text-[var(--color-text-secondary)] uppercase tracking-wider whitespace-nowrap">
                Actions
              </th>
            </tr>
          </thead>
          <tbody>
            {visible.length === 0 ? (
              <tr>
                <td
                  colSpan={COLUMNS.length + 1}
                  className="px-4 py-8 text-center text-sm text-[var(--color-text-secondary)]"
                >
                  {search
                    ? "No devices match your search."
                    : "No devices registered yet."}
                </td>
              </tr>
            ) : (
              visible.map((device) => (
                <tr
                  key={device.id}
                  onClick={() => setSelectedDevice(device)}
                  className="border-t border-[var(--color-border)] cursor-pointer hover:bg-[var(--color-bg-secondary)] transition-colors"
                >
                  {/* Name */}
                  <td className="px-4 py-2.5 whitespace-nowrap">
                    <span className="font-medium">{device.name}</span>
                    <span className="ml-2 text-xs text-[var(--color-text-secondary)]">
                      {device.id}
                    </span>
                  </td>
                  {/* Role */}
                  <td className="px-4 py-2.5 whitespace-nowrap">
                    <RoleBadge role={device.role} />
                  </td>
                  {/* Status */}
                  <td className="px-4 py-2.5 whitespace-nowrap">
                    <span className="inline-flex items-center gap-1.5">
                      <span
                        className={`w-2 h-2 rounded-full ${
                          device.status === "online"
                            ? "bg-emerald-500"
                            : "bg-gray-400"
                        }`}
                      />
                      <span>
                        {device.status === "online" ? "Online" : "Offline"}
                      </span>
                      {device.latency_ms != null && (
                        <LatencyBadge ms={device.latency_ms} />
                      )}
                    </span>
                  </td>
                  {/* Policy */}
                  <td className="px-4 py-2.5 whitespace-nowrap">
                    <select
                      value={device.policy}
                      onChange={(e) => {
                        e.stopPropagation();
                        handleChangePolicy(
                          device.id,
                          e.target.value as PolicyMode,
                        );
                      }}
                      onClick={(e) => e.stopPropagation()}
                      disabled={actionPending === device.id}
                      className="text-xs bg-[var(--color-bg-secondary)] border border-[var(--color-border)] rounded px-1.5 py-0.5 text-[var(--color-text-primary)] focus:outline-none focus:ring-1 focus:ring-brand-500 disabled:opacity-40"
                    >
                      {(
                        Object.entries(POLICY_LABELS) as [
                          PolicyMode,
                          string,
                        ][]
                      ).map(([value, label]) => (
                        <option key={value} value={value}>
                          {label}
                        </option>
                      ))}
                    </select>
                  </td>
                  {/* Last seen */}
                  <td className="px-4 py-2.5 whitespace-nowrap text-[var(--color-text-secondary)]">
                    {fmtRelative(device.last_seen)}
                  </td>
                  {/* Paired */}
                  <td className="px-4 py-2.5 whitespace-nowrap text-[var(--color-text-secondary)]">
                    {fmtDate(device.paired_at)}
                  </td>
                  {/* Actions */}
                  <td className="px-4 py-2.5 whitespace-nowrap text-right">
                    <div
                      className="inline-flex items-center gap-1"
                      onClick={(e) => e.stopPropagation()}
                    >
                      <button
                        onClick={() => handleKick(device.id)}
                        disabled={
                          actionPending === device.id ||
                          device.status !== "online"
                        }
                        title="Kick session"
                        className="px-2 py-1 text-xs rounded border border-[var(--color-border)] text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)] hover:bg-[var(--color-bg-secondary)] transition-colors disabled:opacity-40"
                      >
                        Kick
                      </button>
                      {confirmRevoke === device.id ? (
                        <span className="inline-flex items-center gap-1">
                          <button
                            onClick={() => handleRevoke(device.id)}
                            disabled={actionPending === device.id}
                            className="px-2 py-1 text-xs rounded bg-red-600 text-white hover:bg-red-700 transition-colors disabled:opacity-40"
                          >
                            Confirm
                          </button>
                          <button
                            onClick={() => setConfirmRevoke(null)}
                            className="px-2 py-1 text-xs rounded border border-[var(--color-border)] text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)]"
                          >
                            Cancel
                          </button>
                        </span>
                      ) : (
                        <button
                          onClick={() => setConfirmRevoke(device.id)}
                          disabled={actionPending === device.id}
                          title="Revoke device"
                          className="px-2 py-1 text-xs rounded border border-red-300 dark:border-red-800 text-red-600 dark:text-red-400 hover:bg-red-50 dark:hover:bg-red-950 transition-colors disabled:opacity-40"
                        >
                          Revoke
                        </button>
                      )}
                    </div>
                  </td>
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>

      {/* Detail drawer */}
      <DeviceDetailDrawer
        device={selectedDevice}
        onClose={() => setSelectedDevice(null)}
        onDeviceUpdated={loadDevices}
      />
    </div>
  );
}

// ---------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------

function SortArrow({ dir }: { dir: SortDir }) {
  return (
    <svg
      className={`w-3 h-3 transition-transform ${
        dir === "desc" ? "rotate-180" : ""
      }`}
      viewBox="0 0 12 12"
      fill="currentColor"
    >
      <path d="M6 2l4 5H2z" />
    </svg>
  );
}

function LatencyBadge({ ms }: { ms: number }) {
  let color =
    "text-emerald-600 dark:text-emerald-400 bg-emerald-50 dark:bg-emerald-950";
  if (ms > 100)
    color = "text-red-600 dark:text-red-400 bg-red-50 dark:bg-red-950";
  else if (ms > 50)
    color =
      "text-amber-600 dark:text-amber-400 bg-amber-50 dark:bg-amber-950";

  return (
    <span
      className={`text-[10px] px-1.5 py-0.5 rounded-full font-medium ${color}`}
    >
      {ms}ms
    </span>
  );
}

function RoleBadge({ role }: { role: string }) {
  const styles: Record<string, string> = {
    admin:
      "text-purple-700 dark:text-purple-400 bg-purple-50 dark:bg-purple-950 border-purple-200 dark:border-purple-800",
    member:
      "text-blue-700 dark:text-blue-400 bg-blue-50 dark:bg-blue-950 border-blue-200 dark:border-blue-800",
    guest:
      "text-gray-600 dark:text-gray-400 bg-gray-50 dark:bg-gray-900 border-gray-200 dark:border-gray-700",
  };
  return (
    <span
      className={`text-[10px] px-1.5 py-0.5 rounded border font-medium capitalize ${
        styles[role] ?? styles.guest
      }`}
    >
      {role}
    </span>
  );
}
