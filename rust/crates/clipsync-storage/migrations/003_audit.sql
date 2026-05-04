CREATE TABLE IF NOT EXISTS audit (
    id               TEXT    PRIMARY KEY NOT NULL,
    ts               TEXT    NOT NULL,
    event_type       TEXT    NOT NULL,
    device_id        TEXT    NOT NULL,
    payload_summary  TEXT,
    metadata_json    TEXT
);

CREATE INDEX idx_audit_ts ON audit(ts);
CREATE INDEX idx_audit_event_type ON audit(event_type);
CREATE INDEX idx_audit_device_id ON audit(device_id);
