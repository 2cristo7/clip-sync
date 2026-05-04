# ClipSync Enterprise — Security Considerations

This document describes the security architecture and best practices for ClipSync Enterprise deployments.

---

## TLS Encryption

### Bring Your Own Certificate

For production deployments, configure TLS with your own certificate and private key in the server configuration file (`config.toml`):

```toml
[tls]
enabled = true
cert_path = "/etc/clipsync/certs/server.crt"
key_path = "/etc/clipsync/certs/server.key"
```

The server supports PEM-encoded certificates. You can use certificates from any CA, including Let's Encrypt, your organization's internal CA, or a commercially issued certificate.

After updating the TLS configuration, restart the server:

```bash
sudo systemctl restart clipsync-enterprise
```

### Self-Signed Certificates for Development

When TLS is enabled but no certificate paths are provided, the server generates a self-signed certificate automatically. This is intended for development and testing only.

> **Warning:** Self-signed certificates trigger trust warnings on clients. Do not use self-signed certificates in production.

---

## Authentication and Token Security

### Token Lifecycle

1. **Pairing** — When a device pairs with the server, it receives a unique device token.
2. **Usage** — The token is sent with every request to authenticate the device.
3. **Storage** — The server stores a **hash** of each token, never the raw token itself. If the server database is compromised, raw tokens cannot be extracted.
4. **Expiry** — Tokens expire after a configurable number of days (`auth.token_expiry_days`, default 90). Expired tokens require the device to re-pair.

### Token Rotation

To rotate a device token:

1. Open the admin dashboard and navigate to **Devices**.
2. Select the device and click **Revoke**.
3. On the client machine, re-pair with the server. A new token is issued.

This invalidates the old token immediately. There is no window where both old and new tokens are valid.

### Admin Token

The admin token is generated on the server's first start and printed to the server log. It is used to authenticate the admin dashboard. Store it securely — it grants full administrative access.

To regenerate the admin token, delete the token file from the server data directory and restart the server.

---

## Audit Log Privacy

The audit log records operational metadata for compliance and troubleshooting. It intentionally excludes sensitive content:

| Recorded | Not Recorded |
|---|---|
| Timestamp of each event | Raw clipboard text |
| Device ID and name | File contents |
| Event type (sync, pair, policy change) | Image data |
| Content type (text, image, file) | Passwords or secrets from clipboard |
| Payload byte size | |

This design ensures that even if the audit log is accessed by unauthorized parties, no clipboard content is exposed.

### Audit Retention

Audit entries are automatically pruned after a configurable retention period:

```toml
[audit]
audit_retention_days = 30
```

Set this value according to your organization's compliance requirements. Entries older than the retention period are permanently deleted during the daily pruning cycle.

---

## Network Security

### Port Exposure

The server listens on port **7010** by default. In production:

- Restrict access to this port using firewall rules to only allow known client and dashboard IPs.
- Place the server behind a reverse proxy (e.g. nginx, Caddy) for additional TLS termination, rate limiting, and access control.

### LAN vs. Tailscale

- **LAN deployments** — All traffic stays on the local network. Ensure the network is trusted.
- **Tailscale deployments** — Traffic is encrypted end-to-end by Tailscale's WireGuard tunnels. mDNS discovery does not work over Tailscale (no multicast); devices must use the server URL directly.

---

## Data at Rest

- **Device tokens** — Stored as hashes (never plaintext).
- **Audit logs** — Stored in the server data directory. Contain metadata only (no clipboard content).
- **Clipboard payloads** — Relayed in real time over WebSocket. The server does not persist clipboard content to disk.
- **Configuration file** — Contains sensitive values (TLS key paths, bind address). Restrict file permissions:

  ```bash
  sudo chmod 600 /etc/clipsync/config.toml
  sudo chown root:root /etc/clipsync/config.toml
  ```

---

## Security Best Practices

1. **Always enable TLS in production** with a valid certificate from a trusted CA.
2. **Rotate device tokens** periodically by revoking and re-pairing devices.
3. **Set audit retention** to match your compliance requirements.
4. **Restrict config file permissions** to prevent unauthorized reads.
5. **Use firewall rules** to limit access to port 7010.
6. **Monitor the health endpoint** (`/health`) to detect outages.
7. **Store the admin token securely** — treat it like a root password.
8. **Review audit logs regularly** for unexpected pairing or policy changes.
