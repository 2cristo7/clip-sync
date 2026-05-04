import { useState, useCallback } from "react";
import { ConfirmModal } from "./ConfirmModal";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type PolicyMode =
  | "read_write"
  | "read_only"
  | "write_only"
  | "muted"
  | "follow_leader";

export interface Policy {
  mode: PolicyMode;
  leader_device_id?: string;
}

export interface Device {
  id: string;
  name: string;
  platform: string;
  policy: Policy;
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const POLICY_LABELS: Record<PolicyMode, string> = {
  read_write: "Read / Write",
  read_only: "Read Only",
  write_only: "Write Only",
  muted: "Muted",
  follow_leader: "Follow Leader",
};

const POLICY_BADGE_CLASSES: Record<PolicyMode, string> = {
  read_write:
    "bg-emerald-100 text-emerald-800 dark:bg-emerald-900/40 dark:text-emerald-400",
  read_only:
    "bg-sky-100 text-sky-800 dark:bg-sky-900/40 dark:text-sky-400",
  write_only:
    "bg-amber-100 text-amber-800 dark:bg-amber-900/40 dark:text-amber-400",
  muted:
    "bg-red-100 text-red-800 dark:bg-red-900/40 dark:text-red-400",
  follow_leader:
    "bg-violet-100 text-violet-800 dark:bg-violet-900/40 dark:text-violet-400",
};

// ---------------------------------------------------------------------------
// API helper
// ---------------------------------------------------------------------------

async function applyPolicy(deviceId: string, policy: Policy): Promise<void> {
  const res = await fetch(`/api/devices/${deviceId}/policy`, {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(policy),
  });
  if (!res.ok) {
    throw new Error(`Failed to update policy for ${deviceId}: ${res.statusText}`);
  }
}

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

interface PolicySelectorProps {
  policy: Policy;
  devices: Device[];
  currentDeviceId: string;
  onChange: (p: Policy) => void;
  compact?: boolean;
}

function PolicySelector({
  policy,
  devices,
  currentDeviceId,
  onChange,
  compact = false,
}: PolicySelectorProps) {
  const otherDevices = devices.filter((d) => d.id !== currentDeviceId);

  return (
    <div className={`flex items-center gap-2 ${compact ? "" : "flex-wrap"}`}>
      <select
        value={policy.mode}
        onChange={(e) => {
          const mode = e.target.value as PolicyMode;
          if (mode === "follow_leader") {
            onChange({
              mode,
              leader_device_id: otherDevices[0]?.id ?? "",
            });
          } else {
            onChange({ mode });
          }
        }}
        className="text-xs bg-[var(--color-bg-primary)] border border-[var(--color-border)] text-[var(--color-text-primary)] rounded-md px-2 py-1.5 focus:outline-none focus:ring-1 focus:ring-brand-500 cursor-pointer"
      >
        {(Object.keys(POLICY_LABELS) as PolicyMode[]).map((m) => (
          <option key={m} value={m}>
            {POLICY_LABELS[m]}
          </option>
        ))}
      </select>

      {policy.mode === "follow_leader" && (
        <select
          value={policy.leader_device_id ?? ""}
          onChange={(e) =>
            onChange({ mode: "follow_leader", leader_device_id: e.target.value })
          }
          className="text-xs bg-[var(--color-bg-primary)] border border-[var(--color-border)] text-[var(--color-text-primary)] rounded-md px-2 py-1.5 focus:outline-none focus:ring-1 focus:ring-brand-500 cursor-pointer"
        >
          {otherDevices.length === 0 && (
            <option value="" disabled>
              No other devices
            </option>
          )}
          {otherDevices.map((d) => (
            <option key={d.id} value={d.id}>
              {d.name}
            </option>
          ))}
        </select>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// PolicyEditor (main export)
// ---------------------------------------------------------------------------

interface PolicyEditorProps {
  devices: Device[];
  onDevicesChange?: (devices: Device[]) => void;
}

export function PolicyEditor({ devices, onDevicesChange }: PolicyEditorProps) {
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [editingId, setEditingId] = useState<string | null>(null);
  const [pendingPolicy, setPendingPolicy] = useState<Policy | null>(null);
  const [bulkPolicy, setBulkPolicy] = useState<Policy>({ mode: "read_write" });

  // Confirm modal state
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [confirmAction, setConfirmAction] = useState<{
    title: string;
    message: string;
    action: () => void;
  } | null>(null);

  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // ------- selection helpers -------
  const allSelected =
    devices.length > 0 && selected.size === devices.length;

  const toggleAll = () => {
    if (allSelected) {
      setSelected(new Set());
    } else {
      setSelected(new Set(devices.map((d) => d.id)));
    }
  };

  const toggleOne = (id: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  // ------- apply policy with optional confirm -------
  const needsConfirmation = (policy: Policy) => policy.mode === "muted";

  const doApply = useCallback(
    async (deviceIds: string[], policy: Policy) => {
      setSaving(true);
      setError(null);
      try {
        await Promise.all(deviceIds.map((id) => applyPolicy(id, policy)));
        if (onDevicesChange) {
          const updated = devices.map((d) =>
            deviceIds.includes(d.id) ? { ...d, policy } : d,
          );
          onDevicesChange(updated);
        }
        setEditingId(null);
        setPendingPolicy(null);
      } catch (err) {
        setError(err instanceof Error ? err.message : "Unknown error");
      } finally {
        setSaving(false);
      }
    },
    [devices, onDevicesChange],
  );

  const requestApply = (deviceIds: string[], policy: Policy) => {
    if (needsConfirmation(policy)) {
      setConfirmAction({
        title: "Mute device" + (deviceIds.length > 1 ? "s" : ""),
        message: `Setting ${deviceIds.length > 1 ? "these devices" : "this device"} to Muted will stop all clipboard sync. Continue?`,
        action: () => doApply(deviceIds, policy),
      });
      setConfirmOpen(true);
    } else {
      void doApply(deviceIds, policy);
    }
  };

  // ------- bulk apply -------
  const handleBulkApply = () => {
    if (selected.size === 0) return;
    requestApply([...selected], bulkPolicy);
  };

  // ------- per-device inline edit -------
  const startEditing = (device: Device) => {
    setEditingId(device.id);
    setPendingPolicy({ ...device.policy });
  };

  const commitEdit = (deviceId: string) => {
    if (!pendingPolicy) return;
    requestApply([deviceId], pendingPolicy);
  };

  const cancelEdit = () => {
    setEditingId(null);
    setPendingPolicy(null);
  };

  // ------- render -------
  if (devices.length === 0) {
    return (
      <div className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-secondary)] p-8 text-center text-sm text-[var(--color-text-secondary)]">
        No devices registered yet. Connect the enterprise server to see devices.
      </div>
    );
  }

  return (
    <>
      {/* Bulk bar */}
      {selected.size > 0 && (
        <div className="flex items-center gap-3 mb-3 p-3 rounded-lg border border-brand-500/30 bg-brand-50/50 dark:bg-brand-900/20">
          <span className="text-xs font-medium text-[var(--color-text-primary)]">
            {selected.size} selected
          </span>
          <div className="h-4 w-px bg-[var(--color-border)]" />
          <PolicySelector
            policy={bulkPolicy}
            devices={devices}
            currentDeviceId=""
            onChange={setBulkPolicy}
            compact
          />
          <button
            type="button"
            onClick={handleBulkApply}
            disabled={saving}
            className="ml-auto px-3 py-1.5 text-xs font-medium rounded-md bg-brand-600 text-white hover:bg-brand-700 disabled:opacity-50 transition-colors"
          >
            Apply to all
          </button>
        </div>
      )}

      {/* Error banner */}
      {error && (
        <div className="mb-3 px-4 py-2 rounded-md bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 text-xs text-red-700 dark:text-red-400">
          {error}
        </div>
      )}

      {/* Table */}
      <div className="overflow-x-auto rounded-lg border border-[var(--color-border)]">
        <table className="w-full text-xs">
          <thead>
            <tr className="border-b border-[var(--color-border)] bg-[var(--color-bg-secondary)]">
              <th className="w-8 px-3 py-2">
                <input
                  type="checkbox"
                  checked={allSelected}
                  onChange={toggleAll}
                  className="accent-brand-600 cursor-pointer"
                />
              </th>
              <th className="px-3 py-2 text-left font-medium text-[var(--color-text-secondary)] uppercase tracking-wider">
                Device
              </th>
              <th className="px-3 py-2 text-left font-medium text-[var(--color-text-secondary)] uppercase tracking-wider">
                Platform
              </th>
              <th className="px-3 py-2 text-left font-medium text-[var(--color-text-secondary)] uppercase tracking-wider">
                Policy
              </th>
              <th className="px-3 py-2 text-right font-medium text-[var(--color-text-secondary)] uppercase tracking-wider">
                Actions
              </th>
            </tr>
          </thead>
          <tbody className="divide-y divide-[var(--color-border)]">
            {devices.map((device) => {
              const isEditing = editingId === device.id;
              return (
                <tr
                  key={device.id}
                  className={`transition-colors ${
                    selected.has(device.id)
                      ? "bg-brand-50/40 dark:bg-brand-900/10"
                      : "bg-[var(--color-bg-primary)] hover:bg-[var(--color-bg-secondary)]"
                  }`}
                >
                  <td className="px-3 py-2.5">
                    <input
                      type="checkbox"
                      checked={selected.has(device.id)}
                      onChange={() => toggleOne(device.id)}
                      className="accent-brand-600 cursor-pointer"
                    />
                  </td>
                  <td className="px-3 py-2.5 font-medium text-[var(--color-text-primary)] whitespace-nowrap">
                    {device.name}
                    <span className="ml-2 text-[10px] text-[var(--color-text-secondary)] font-normal">
                      {device.id.slice(0, 8)}
                    </span>
                  </td>
                  <td className="px-3 py-2.5 text-[var(--color-text-secondary)] whitespace-nowrap">
                    {device.platform}
                  </td>
                  <td className="px-3 py-2.5">
                    {isEditing && pendingPolicy ? (
                      <PolicySelector
                        policy={pendingPolicy}
                        devices={devices}
                        currentDeviceId={device.id}
                        onChange={setPendingPolicy}
                        compact
                      />
                    ) : (
                      <span
                        className={`inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[10px] font-semibold ${POLICY_BADGE_CLASSES[device.policy.mode]}`}
                      >
                        {POLICY_LABELS[device.policy.mode]}
                        {device.policy.mode === "follow_leader" &&
                          device.policy.leader_device_id && (
                            <span className="font-normal opacity-75">
                              {" "}
                              ({devices.find(
                                (d) =>
                                  d.id === device.policy.leader_device_id,
                              )?.name ?? device.policy.leader_device_id.slice(0, 8)})
                            </span>
                          )}
                      </span>
                    )}
                  </td>
                  <td className="px-3 py-2.5 text-right whitespace-nowrap">
                    {isEditing ? (
                      <div className="flex items-center justify-end gap-1.5">
                        <button
                          type="button"
                          onClick={() => commitEdit(device.id)}
                          disabled={saving}
                          className="px-2.5 py-1 rounded-md bg-brand-600 text-white hover:bg-brand-700 disabled:opacity-50 transition-colors"
                        >
                          Save
                        </button>
                        <button
                          type="button"
                          onClick={cancelEdit}
                          className="px-2.5 py-1 rounded-md border border-[var(--color-border)] text-[var(--color-text-secondary)] hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors"
                        >
                          Cancel
                        </button>
                      </div>
                    ) : (
                      <button
                        type="button"
                        onClick={() => startEditing(device)}
                        className="px-2.5 py-1 rounded-md border border-[var(--color-border)] text-[var(--color-text-secondary)] hover:bg-gray-100 dark:hover:bg-gray-800 hover:text-[var(--color-text-primary)] transition-colors"
                      >
                        Edit
                      </button>
                    )}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>

      {/* Confirm modal */}
      <ConfirmModal
        open={confirmOpen}
        title={confirmAction?.title ?? ""}
        message={confirmAction?.message ?? ""}
        confirmLabel="Yes, apply"
        destructive
        onConfirm={() => {
          setConfirmOpen(false);
          confirmAction?.action();
        }}
        onCancel={() => setConfirmOpen(false)}
      />
    </>
  );
}
