/** Event types emitted by the enterprise audit system. */
export type AuditEventType =
  | "device_paired"
  | "device_revoked"
  | "clipboard_pushed"
  | "clipboard_delivered"
  | "broadcast_sent"
  | "broadcast_delivered"
  | "policy_changed"
  | "connection_opened"
  | "connection_closed";

export const AUDIT_EVENT_LABELS: Record<AuditEventType, string> = {
  device_paired: "Device Paired",
  device_revoked: "Device Revoked",
  clipboard_pushed: "Clipboard Pushed",
  clipboard_delivered: "Clipboard Delivered",
  broadcast_sent: "Broadcast Sent",
  broadcast_delivered: "Broadcast Delivered",
  policy_changed: "Policy Changed",
  connection_opened: "Connection Opened",
  connection_closed: "Connection Closed",
};

export const ALL_EVENT_TYPES: AuditEventType[] = Object.keys(
  AUDIT_EVENT_LABELS,
) as AuditEventType[];

/** A single audit log entry returned by GET /audit. */
export interface AuditEntry {
  id: string;
  /** ISO-8601 timestamp */
  timestamp: string;
  event_type: AuditEventType;
  device_id: string;
  device_name: string;
  detail: string;
  actor: string;
}

/** WS message for real-time audit streaming. */
export interface AuditStreamMessage {
  type: "audit_event";
  entry: AuditEntry;
}

/** Query parameters for GET /audit. */
export interface AuditQuery {
  from?: string;
  to?: string;
  device_id?: string;
  event_type?: AuditEventType;
  limit?: number;
}
