import { useEffect, useState } from "react";
import type { Device, AuditEvent, PolicyMode } from "../types/device";
import { POLICY_LABELS } from "../types/device";
import {
  fetchAuditEvents,
  changePolicy,
  revokeDevice,
  kickSession,
} from "../api/client";

interface DeviceDetailDrawerProps {
  device: Device | null;
  onClose: () => void;
  onDeviceUpdated: () => void;
}

export function DeviceDetailDrawer({
  device,
  onClose,
  onDeviceUpdated,
}: DeviceDetailDrawerProps) {
  const [events, setEvents] = useState<AuditEvent[]>([]);
  const [loading, setLoading] = useState(false);
  const [actionPending, setActionPending] = useState(false);

  useEffect(() => {
    if (!device) {
      setEvents([]);
      return;
    }
    let cancelled = false;
    setLoading(true);
    fetchAuditEvents(device.id, 50).then((data) => {
      if (!cancelled) {
        setEvents(data);
        setLoading(false);
      }
    });
    return () => {
      cancelled = true;
    };
  }, [device]);

  if (!device) return null;

  const isOnline = device.status === "online";

  async function handlePolicyChange(newPolicy: PolicyMode) {
    if (!device) return;
    setActionPending(true);
    await changePolicy(device.id, newPolicy);
    onDeviceUpdated();
    setActionPending(false);
  }

  async function handleRevoke() {
    if (!device) return;
    if (
      !window.confirm(
        `Revoke device "${device.name}"? This cannot be undone.`,
      )
    )
      return;
    setActionPending(true);
    await revokeDevice(device.id);
    onDeviceUpdated();
    onClose();
    setActionPending(false);
  }

  async function handleKick() {
    if (!device) return;
    setActionPending(true);
    await kickSession(device.id);
    onDeviceUpdated();
    setActionPending(false);
  }

  function fmtTime(iso: string) {
    const d = new Date(iso);
    return d.toLocaleString(undefined, {
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
    });
  }

  return (
    <>
      {/* Backdrop */}
      <div
        className="fixed inset-0 z-40 bg-black/30 dark:bg-black/50"
        onClick={onClose}
      />

      {/* Drawer */}
      <aside className="fixed inset-y-0 right-0 z-50 w-[420px] max-w-full flex flex-col bg-[var(--color-bg-primary)] border-l border-[var(--color-border)] shadow-xl">
        {/* Header */}
        <div className="flex items-center justify-between px-5 py-4 border-b border-[var(--color-border)]">
          <h2 className="text-sm font-semibold truncate">{device.name}</h2>
          <button
            onClick={onClose}
            className="text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)] transition-colors"
          >
            <svg
              className="w-4 h-4"
              viewBox="0 0 16 16"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
            >
              <path d="M4 4l8 8M12 4l-8 8" />
            </svg>
          </button>
        </div>

        {/* Info grid */}
        <div className="px-5 py-4 border-b border-[var(--color-border)] space-y-3 text-sm">
          <div className="grid grid-cols-2 gap-x-4 gap-y-2">
            <InfoRow label="ID" value={device.id} />
            <InfoRow label="OS" value={device.os} />
            <InfoRow label="IP" value={device.ip} />
            <InfoRow label="Version" value={device.version} />
            <InfoRow label="Role" value={device.role} />
            <InfoRow label="Paired" value={fmtTime(device.paired_at)} />
            <InfoRow label="Last seen" value={fmtTime(device.last_seen)} />
            <InfoRow
              label="Status"
              value={
                <span className="flex items-center gap-1.5">
                  <span
                    className={`w-2 h-2 rounded-full ${
                      isOnline ? "bg-emerald-500" : "bg-gray-400"
                    }`}
                  />
                  {isOnline ? "Online" : "Offline"}
                  {device.latency_ms != null && (
                    <span className="text-[var(--color-text-secondary)]">
                      ({device.latency_ms}ms)
                    </span>
                  )}
                </span>
              }
            />
          </div>
        </div>

        {/* Actions */}
        <div className="px-5 py-3 border-b border-[var(--color-border)] flex items-center gap-2 text-xs">
          <label className="text-[var(--color-text-secondary)] mr-1">
            Policy
          </label>
          <select
            value={device.policy}
            onChange={(e) =>
              handlePolicyChange(e.target.value as PolicyMode)
            }
            disabled={actionPending}
            className="bg-[var(--color-bg-secondary)] border border-[var(--color-border)] rounded px-2 py-1 text-xs text-[var(--color-text-primary)] focus:outline-none focus:ring-1 focus:ring-brand-500"
          >
            {(Object.entries(POLICY_LABELS) as [PolicyMode, string][]).map(
              ([value, label]) => (
                <option key={value} value={value}>
                  {label}
                </option>
              ),
            )}
          </select>
          <div className="ml-auto flex items-center gap-2">
            <button
              onClick={handleKick}
              disabled={actionPending || !isOnline}
              className="px-2.5 py-1 rounded border border-[var(--color-border)] text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)] hover:bg-[var(--color-bg-secondary)] transition-colors disabled:opacity-40"
            >
              Kick
            </button>
            <button
              onClick={handleRevoke}
              disabled={actionPending}
              className="px-2.5 py-1 rounded border border-red-300 dark:border-red-800 text-red-600 dark:text-red-400 hover:bg-red-50 dark:hover:bg-red-950 transition-colors disabled:opacity-40"
            >
              Revoke
            </button>
          </div>
        </div>

        {/* Activity log */}
        <div className="flex-1 overflow-y-auto px-5 py-3">
          <h3 className="text-xs font-medium text-[var(--color-text-secondary)] uppercase tracking-wider mb-2">
            Recent Activity
          </h3>
          {loading ? (
            <p className="text-xs text-[var(--color-text-secondary)]">
              Loading...
            </p>
          ) : events.length === 0 ? (
            <p className="text-xs text-[var(--color-text-secondary)]">
              No activity recorded.
            </p>
          ) : (
            <ul className="space-y-1.5">
              {events.map((evt) => (
                <li
                  key={evt.id}
                  className="flex items-start gap-2 text-xs leading-relaxed"
                >
                  <span className="shrink-0 w-[110px] text-[var(--color-text-secondary)] tabular-nums">
                    {fmtTime(evt.timestamp)}
                  </span>
                  <span className="font-medium min-w-[100px]">
                    {evt.action}
                  </span>
                  <span className="text-[var(--color-text-secondary)] truncate">
                    {evt.detail}
                  </span>
                </li>
              ))}
            </ul>
          )}
        </div>
      </aside>
    </>
  );
}

function InfoRow({
  label,
  value,
}: {
  label: string;
  value: React.ReactNode;
}) {
  return (
    <div className="flex flex-col">
      <span className="text-[var(--color-text-secondary)] text-xs">
        {label}
      </span>
      <span className="text-[var(--color-text-primary)] text-sm truncate">
        {value}
      </span>
    </div>
  );
}
