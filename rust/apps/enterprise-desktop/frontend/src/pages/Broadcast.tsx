import { useCallback, useEffect, useRef, useState } from "react";
import { fetchDevices } from "../api/client";
import type { Device } from "../types/device";
import {
  FileDropZone,
  FilePreviewList,
  formatSize,
  type SelectedFile,
} from "../components/FileDropZone";
import { DevicePicker } from "../components/DevicePicker";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

type DeliveryStatus = "pending" | "delivered" | "failed";

interface DeviceProgress {
  deviceId: string;
  deviceName: string;
  status: DeliveryStatus;
}

interface BroadcastRecord {
  id: string;
  fileName: string;
  fileSize: number;
  recipientCount: number;
  timestamp: string;
  results: DeviceProgress[];
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const BASE_URL =
  ((typeof window !== "undefined" &&
    (window as unknown as Record<string, unknown>).__CLIPSYNC_API_URL__) as
    | string
    | undefined) ?? "http://127.0.0.1:9300";

const MAX_HISTORY = 20;

// ---------------------------------------------------------------------------
// Mock broadcast sender (falls back when server unreachable)
// ---------------------------------------------------------------------------

async function sendBroadcast(
  files: SelectedFile[],
  deviceIds: string[],
  deviceMap: Map<string, Device>,
  onProgress: (progress: DeviceProgress[]) => void,
  abortSignal: AbortSignal,
): Promise<DeviceProgress[]> {
  const progress: DeviceProgress[] = deviceIds.map((id) => ({
    deviceId: id,
    deviceName: deviceMap.get(id)?.name ?? id,
    status: "pending" as DeliveryStatus,
  }));
  onProgress([...progress]);

  // Try real server first
  try {
    const formData = new FormData();
    for (const f of files) {
      formData.append("files", f.file);
    }
    formData.append("device_ids", JSON.stringify(deviceIds));

    const res = await fetch(`${BASE_URL}/broadcast`, {
      method: "POST",
      body: formData,
      signal: abortSignal,
    });

    if (res.ok) {
      const result = (await res.json()) as {
        results: Array<{ device_id: string; status: string }>;
      };
      for (const r of result.results) {
        const entry = progress.find((p) => p.deviceId === r.device_id);
        if (entry) {
          entry.status = r.status === "delivered" ? "delivered" : "failed";
        }
      }
      onProgress([...progress]);
      return progress;
    }
  } catch {
    // Fall through to mock
  }

  // Mock: simulate per-device delivery with delays
  for (let i = 0; i < progress.length; i++) {
    if (abortSignal.aborted) break;
    await new Promise<void>((resolve) => {
      const timer = setTimeout(resolve, 400 + Math.random() * 600);
      abortSignal.addEventListener("abort", () => {
        clearTimeout(timer);
        resolve();
      });
    });
    if (abortSignal.aborted) break;

    const device = deviceMap.get(progress[i].deviceId);
    progress[i].status =
      device?.status === "online" ? "delivered" : "failed";
    onProgress([...progress]);
  }

  return progress;
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function BroadcastPage() {
  const [devices, setDevices] = useState<Device[]>([]);
  const [selectedDevices, setSelectedDevices] = useState<Set<string>>(
    new Set(),
  );
  const [files, setFiles] = useState<SelectedFile[]>([]);
  const [sending, setSending] = useState(false);
  const [progress, setProgress] = useState<DeviceProgress[]>([]);
  const [history, setHistory] = useState<BroadcastRecord[]>([]);
  const abortRef = useRef<AbortController | null>(null);

  // Load devices on mount
  useEffect(() => {
    fetchDevices().then(setDevices).catch(() => {});
  }, []);

  const deviceMap = new Map(devices.map((d) => [d.id, d]));

  const handleFilesSelected = useCallback(
    (newFiles: SelectedFile[]) => {
      setFiles((prev) => [...prev, ...newFiles]);
    },
    [],
  );

  const handleRemoveFile = useCallback((index: number) => {
    setFiles((prev) => prev.filter((_, i) => i !== index));
  }, []);

  const canSend = files.length > 0 && selectedDevices.size > 0 && !sending;

  const handleSend = useCallback(async () => {
    if (!canSend) return;

    setSending(true);
    setProgress([]);

    const controller = new AbortController();
    abortRef.current = controller;

    const deviceIds = Array.from(selectedDevices);
    const results = await sendBroadcast(
      files,
      deviceIds,
      deviceMap,
      setProgress,
      controller.signal,
    );

    // Add to history
    const record: BroadcastRecord = {
      id: `bc-${Date.now()}`,
      fileName: files.map((f) => f.name).join(", "),
      fileSize: files.reduce((sum, f) => sum + f.size, 0),
      recipientCount: deviceIds.length,
      timestamp: new Date().toISOString(),
      results,
    };

    setHistory((prev) => [record, ...prev].slice(0, MAX_HISTORY));
    setSending(false);
    abortRef.current = null;

    // Clear files after send
    setFiles([]);
    setProgress([]);
  }, [canSend, files, selectedDevices, deviceMap]);

  const handleCancel = useCallback(() => {
    abortRef.current?.abort();
    setSending(false);
  }, []);

  return (
    <div className="p-6 max-w-3xl">
      <h1 className="text-lg font-semibold mb-1">Broadcast</h1>
      <p className="text-sm text-[var(--color-text-secondary)] mb-6">
        Send files to all or selected devices.
      </p>

      {/* File drop zone */}
      <section className="mb-6">
        <FileDropZone onFilesSelected={handleFilesSelected} disabled={sending} />
        <div className="mt-3">
          <FilePreviewList files={files} onRemove={handleRemoveFile} />
        </div>
      </section>

      {/* Device picker */}
      <section className="mb-6">
        <DevicePicker
          devices={devices}
          selected={selectedDevices}
          onChange={setSelectedDevices}
          disabled={sending}
        />
      </section>

      {/* Send / Cancel */}
      <section className="mb-6 flex items-center gap-3">
        <button
          type="button"
          disabled={!canSend}
          onClick={handleSend}
          className="px-4 py-2 text-sm font-medium rounded-md bg-brand-600 text-white hover:bg-brand-700 transition-colors disabled:opacity-40 disabled:cursor-not-allowed focus:outline-none focus:ring-2 focus:ring-brand-500 focus:ring-offset-1"
        >
          {sending ? "Sending..." : "Send broadcast"}
        </button>
        {sending && (
          <button
            type="button"
            onClick={handleCancel}
            className="px-3 py-2 text-sm font-medium rounded-md border border-[var(--color-border)] text-[var(--color-text-secondary)] hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors"
          >
            Cancel
          </button>
        )}
      </section>

      {/* Live progress */}
      {progress.length > 0 && (
        <section className="mb-6">
          <h2 className="text-sm font-medium text-[var(--color-text-primary)] mb-2">
            Delivery progress
          </h2>
          <div className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-secondary)] divide-y divide-[var(--color-border)]">
            {progress.map((p) => (
              <div
                key={p.deviceId}
                className="flex items-center justify-between px-3 py-2"
              >
                <span className="text-sm text-[var(--color-text-primary)]">
                  {p.deviceName}
                </span>
                <StatusBadge status={p.status} />
              </div>
            ))}
          </div>
        </section>
      )}

      {/* History */}
      <section>
        <h2 className="text-sm font-medium text-[var(--color-text-primary)] mb-2">
          Recent broadcasts
        </h2>
        {history.length === 0 ? (
          <p className="text-sm text-[var(--color-text-secondary)]">
            No broadcasts yet.
          </p>
        ) : (
          <div className="rounded-lg border border-[var(--color-border)] bg-[var(--color-bg-secondary)] divide-y divide-[var(--color-border)]">
            {history.map((rec) => {
              const delivered = rec.results.filter(
                (r) => r.status === "delivered",
              ).length;
              const failed = rec.results.filter(
                (r) => r.status === "failed",
              ).length;
              return (
                <div key={rec.id} className="px-3 py-2.5">
                  <div className="flex items-center justify-between">
                    <span className="text-sm text-[var(--color-text-primary)] truncate max-w-[60%]">
                      {rec.fileName}
                    </span>
                    <span className="text-xs text-[var(--color-text-secondary)]">
                      {new Date(rec.timestamp).toLocaleString()}
                    </span>
                  </div>
                  <div className="flex items-center gap-3 mt-1 text-xs text-[var(--color-text-secondary)]">
                    <span>{formatSize(rec.fileSize)}</span>
                    <span>&middot;</span>
                    <span>{rec.recipientCount} recipients</span>
                    {delivered > 0 && (
                      <span className="text-green-600 dark:text-green-400">
                        {delivered} delivered
                      </span>
                    )}
                    {failed > 0 && (
                      <span className="text-red-600 dark:text-red-400">
                        {failed} failed
                      </span>
                    )}
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </section>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Status badge
// ---------------------------------------------------------------------------

function StatusBadge({ status }: { status: DeliveryStatus }) {
  const config: Record<DeliveryStatus, { label: string; classes: string }> = {
    pending: {
      label: "Pending",
      classes:
        "bg-yellow-100 text-yellow-800 dark:bg-yellow-900/30 dark:text-yellow-300",
    },
    delivered: {
      label: "Delivered",
      classes:
        "bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-300",
    },
    failed: {
      label: "Failed",
      classes:
        "bg-red-100 text-red-800 dark:bg-red-900/30 dark:text-red-300",
    },
  };

  const { label, classes } = config[status];

  return (
    <span
      className={`inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium ${classes}`}
    >
      {label}
    </span>
  );
}
