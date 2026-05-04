import { useCallback, useMemo } from "react";
import type { Device } from "../types/device";

interface DevicePickerProps {
  devices: Device[];
  selected: Set<string>;
  onChange: (selected: Set<string>) => void;
  disabled?: boolean;
}

export function DevicePicker({
  devices,
  selected,
  onChange,
  disabled,
}: DevicePickerProps) {
  const onlineIds = useMemo(
    () => devices.filter((d) => d.status === "online").map((d) => d.id),
    [devices],
  );

  const pairedIds = useMemo(() => devices.map((d) => d.id), [devices]);

  const toggleDevice = useCallback(
    (id: string) => {
      const next = new Set(selected);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      onChange(next);
    },
    [selected, onChange],
  );

  const selectPreset = useCallback(
    (ids: string[]) => {
      onChange(new Set(ids));
    },
    [onChange],
  );

  return (
    <div className="space-y-3">
      <div className="flex items-center gap-2">
        <span className="text-sm font-medium text-[var(--color-text-primary)]">
          Target devices
        </span>
        <span className="text-xs text-[var(--color-text-secondary)]">
          ({selected.size} selected)
        </span>
        <div className="ml-auto flex gap-1.5">
          <PresetButton
            label="All online"
            count={onlineIds.length}
            disabled={disabled || onlineIds.length === 0}
            onClick={() => selectPreset(onlineIds)}
          />
          <PresetButton
            label="All paired"
            count={pairedIds.length}
            disabled={disabled || pairedIds.length === 0}
            onClick={() => selectPreset(pairedIds)}
          />
          {selected.size > 0 && (
            <PresetButton
              label="Clear"
              disabled={disabled}
              onClick={() => onChange(new Set())}
            />
          )}
        </div>
      </div>

      <div className="max-h-48 overflow-y-auto rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-secondary)] divide-y divide-[var(--color-border)]">
        {devices.length === 0 ? (
          <p className="px-3 py-4 text-sm text-center text-[var(--color-text-secondary)]">
            No paired devices found
          </p>
        ) : (
          devices.map((device) => (
            <label
              key={device.id}
              className={`flex items-center gap-3 px-3 py-2 cursor-pointer transition-colors hover:bg-gray-100 dark:hover:bg-gray-800/60 ${
                disabled ? "opacity-50 cursor-not-allowed" : ""
              }`}
            >
              <input
                type="checkbox"
                checked={selected.has(device.id)}
                onChange={() => toggleDevice(device.id)}
                disabled={disabled}
                className="rounded border-gray-300 text-brand-600 focus:ring-brand-500"
              />
              <div className="flex-1 min-w-0">
                <p className="text-sm text-[var(--color-text-primary)] truncate">
                  {device.name}
                </p>
                <p className="text-xs text-[var(--color-text-secondary)]">
                  {device.os} &middot; {device.ip}
                </p>
              </div>
              <StatusDot status={device.status} />
            </label>
          ))
        )}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

function PresetButton({
  label,
  count,
  disabled,
  onClick,
}: {
  label: string;
  count?: number;
  disabled?: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      disabled={disabled}
      onClick={onClick}
      className="px-2 py-1 text-xs font-medium rounded-md border border-[var(--color-border)] text-[var(--color-text-secondary)] hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
    >
      {label}
      {count !== undefined && (
        <span className="ml-1 text-[var(--color-text-secondary)]">({count})</span>
      )}
    </button>
  );
}

function StatusDot({ status }: { status: string }) {
  const isOnline = status === "online";
  return (
    <span
      className={`inline-block w-2 h-2 rounded-full shrink-0 ${
        isOnline ? "bg-green-500" : "bg-gray-400"
      }`}
      title={isOnline ? "Online" : "Offline"}
    />
  );
}
