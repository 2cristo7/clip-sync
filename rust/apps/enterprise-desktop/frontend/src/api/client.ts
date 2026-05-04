import type {
  Device,
  AuditEvent,
  PolicyMode,
  DeviceStatusUpdate,
} from "../types/device";

// ------------------------------------------------------------------
// Configuration
// ------------------------------------------------------------------

const BASE_URL =
  (typeof window !== "undefined" && (window as unknown as Record<string, unknown>).__CLIPSYNC_API_URL__) as string | undefined ??
  "http://127.0.0.1:9300";

const WS_URL = BASE_URL.replace(/^http/, "ws") + "/ws/events";

// ------------------------------------------------------------------
// Mock data (used when the enterprise server is not reachable)
// ------------------------------------------------------------------

const MOCK_DEVICES: Device[] = [
  {
    id: "d-001",
    name: "MacBook Pro — Eng",
    role: "admin",
    status: "online",
    latency_ms: 4,
    policy: "full_access",
    last_seen: new Date().toISOString(),
    paired_at: "2026-03-15T09:00:00Z",
    os: "macOS 15.4",
    ip: "192.168.1.10",
    version: "0.1.0",
  },
  {
    id: "d-002",
    name: "Pixel 9 — Design",
    role: "member",
    status: "online",
    latency_ms: 18,
    policy: "send_only",
    last_seen: new Date().toISOString(),
    paired_at: "2026-03-16T11:30:00Z",
    os: "Android 16",
    ip: "192.168.1.42",
    version: "0.1.0",
  },
  {
    id: "d-003",
    name: "ThinkPad X1 — PM",
    role: "member",
    status: "offline",
    latency_ms: null,
    policy: "receive_only",
    last_seen: "2026-05-03T14:22:00Z",
    paired_at: "2026-04-01T08:15:00Z",
    os: "Linux 6.8",
    ip: "192.168.1.55",
    version: "0.1.0",
  },
  {
    id: "d-004",
    name: "iPad Pro — Exec",
    role: "guest",
    status: "offline",
    latency_ms: null,
    policy: "text_only",
    last_seen: "2026-05-01T09:00:00Z",
    paired_at: "2026-04-20T16:00:00Z",
    os: "iPadOS 19",
    ip: "192.168.1.88",
    version: "0.1.0",
  },
  {
    id: "d-005",
    name: "Galaxy S25 — QA",
    role: "member",
    status: "online",
    latency_ms: 12,
    policy: "full_access",
    last_seen: new Date().toISOString(),
    paired_at: "2026-04-25T10:45:00Z",
    os: "Android 16",
    ip: "192.168.1.91",
    version: "0.1.0",
  },
  {
    id: "d-006",
    name: "Mac Mini — CI",
    role: "admin",
    status: "online",
    latency_ms: 2,
    policy: "disabled",
    last_seen: new Date().toISOString(),
    paired_at: "2026-02-10T07:00:00Z",
    os: "macOS 15.4",
    ip: "192.168.1.5",
    version: "0.1.0",
  },
];

function mockAuditEvents(deviceId: string): AuditEvent[] {
  const actions = [
    "clipboard_sync",
    "clipboard_sync",
    "clipboard_sync",
    "policy_changed",
    "session_started",
    "session_ended",
    "pair_verified",
  ];
  return Array.from({ length: 50 }, (_, i) => ({
    id: `audit-${deviceId}-${i}`,
    device_id: deviceId,
    action: actions[i % actions.length],
    detail:
      i % 7 === 3
        ? "Policy changed from full_access to send_only"
        : `Clipboard payload ${i + 1} synced`,
    timestamp: new Date(Date.now() - i * 120_000).toISOString(),
  }));
}

// ------------------------------------------------------------------
// REST helpers
// ------------------------------------------------------------------

async function tryFetch<T>(
  path: string,
  fallback: T,
  init?: RequestInit,
): Promise<T> {
  try {
    const res = await fetch(`${BASE_URL}${path}`, {
      ...init,
      headers: { "Content-Type": "application/json", ...init?.headers },
    });
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    return (await res.json()) as T;
  } catch {
    return fallback;
  }
}

// ------------------------------------------------------------------
// Public API
// ------------------------------------------------------------------

export async function fetchDevices(): Promise<Device[]> {
  return tryFetch("/devices", MOCK_DEVICES);
}

export async function fetchAuditEvents(
  deviceId: string,
  limit = 50,
): Promise<AuditEvent[]> {
  return tryFetch(
    `/audit?device_id=${encodeURIComponent(deviceId)}&limit=${limit}`,
    mockAuditEvents(deviceId),
  );
}

export async function changePolicy(
  deviceId: string,
  policy: PolicyMode,
): Promise<void> {
  await tryFetch(`/devices/${encodeURIComponent(deviceId)}/policy`, undefined, {
    method: "PUT",
    body: JSON.stringify({ policy }),
  });
}

export async function revokeDevice(deviceId: string): Promise<void> {
  await tryFetch(`/devices/${encodeURIComponent(deviceId)}`, undefined, {
    method: "DELETE",
  });
}

export async function kickSession(deviceId: string): Promise<void> {
  await tryFetch(
    `/devices/${encodeURIComponent(deviceId)}/kick`,
    undefined,
    { method: "POST" },
  );
}

// ------------------------------------------------------------------
// WebSocket subscription
// ------------------------------------------------------------------

export function subscribeDeviceStatus(
  onUpdate: (update: DeviceStatusUpdate) => void,
): () => void {
  let ws: WebSocket | null = null;
  let disposed = false;
  let reconnectTimer: ReturnType<typeof setTimeout> | null = null;

  function connect() {
    if (disposed) return;
    try {
      ws = new WebSocket(WS_URL);
    } catch {
      scheduleReconnect();
      return;
    }

    ws.onmessage = (evt) => {
      try {
        const msg = JSON.parse(evt.data as string) as DeviceStatusUpdate;
        if (msg.type === "device_status") {
          onUpdate(msg);
        }
      } catch {
        // Ignore non-JSON or unknown messages
      }
    };

    ws.onclose = () => {
      if (!disposed) scheduleReconnect();
    };

    ws.onerror = () => {
      ws?.close();
    };
  }

  function scheduleReconnect() {
    if (reconnectTimer) return;
    reconnectTimer = setTimeout(() => {
      reconnectTimer = null;
      connect();
    }, 2000);
  }

  connect();

  return () => {
    disposed = true;
    if (reconnectTimer) clearTimeout(reconnectTimer);
    ws?.close();
  };
}
