/** Policy modes available for enterprise devices. */
export type PolicyMode =
  | "full_access"
  | "send_only"
  | "receive_only"
  | "text_only"
  | "disabled";

export const POLICY_LABELS: Record<PolicyMode, string> = {
  full_access: "Full Access",
  send_only: "Send Only",
  receive_only: "Receive Only",
  text_only: "Text Only",
  disabled: "Disabled",
};

export type DeviceRole = "admin" | "member" | "guest";

export type DeviceStatus = "online" | "offline";

export interface Device {
  id: string;
  name: string;
  role: DeviceRole;
  status: DeviceStatus;
  /** Round-trip latency in ms; null when offline. */
  latency_ms: number | null;
  policy: PolicyMode;
  /** ISO-8601 */
  last_seen: string;
  /** ISO-8601 */
  paired_at: string;
  os: string;
  ip: string;
  version: string;
}

export interface AuditEvent {
  id: string;
  device_id: string;
  action: string;
  detail: string;
  /** ISO-8601 */
  timestamp: string;
}

/** WS message sent by the server for device status updates. */
export interface DeviceStatusUpdate {
  type: "device_status";
  device_id: string;
  status: DeviceStatus;
  latency_ms: number | null;
}
