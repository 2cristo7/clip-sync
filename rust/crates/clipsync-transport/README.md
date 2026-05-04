# clipsync-transport

Transport-layer plumbing for ClipSync. Today this is mDNS service
advertisement/discovery (`_clipsync._tcp.local.`) plus the shared
WebSocket / healthcheck tuning constants (`WS_PING_INTERVAL`,
`WS_READ_TIMEOUT`, `HEALTHCHECK_POLL_INTERVAL`,
`CONSECUTIVE_FAILURE_THRESHOLD`) that the server hub and the client
connector must agree on. Future scope: the WebSocket hub itself, plus
network-reachability monitoring.
