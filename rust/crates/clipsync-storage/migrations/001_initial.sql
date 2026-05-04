CREATE TABLE IF NOT EXISTS devices (
    id          TEXT    PRIMARY KEY NOT NULL,
    name        TEXT    NOT NULL,
    fingerprint TEXT    NOT NULL,
    role        TEXT    NOT NULL DEFAULT 'client',
    paired_at   TEXT    NOT NULL,
    last_seen   TEXT    NOT NULL,
    token_hash  TEXT    NOT NULL
);

CREATE TABLE IF NOT EXISTS tokens (
    token_id    TEXT    PRIMARY KEY NOT NULL,
    device_id   TEXT    NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    created_at  TEXT    NOT NULL,
    revoked_at  TEXT
);

CREATE INDEX idx_tokens_device_id ON tokens(device_id);
CREATE INDEX idx_tokens_revoked_at ON tokens(revoked_at);
CREATE INDEX idx_devices_fingerprint ON devices(fingerprint);
