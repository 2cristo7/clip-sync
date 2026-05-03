# Documentation

## Setup

| Document | Description |
|----------|-------------|
| [build-from-source.md](build-from-source.md) | Full setup guide: macOS build, code signing, Android sideload, Shizuku, Tailscale, pairing |

## Architecture

| Document | Description |
|----------|-------------|
| [architecture/protocol.md](architecture/protocol.md) | Wire protocol: endpoints, payload schema, auth |
| [architecture/security.md](architecture/security.md) | Security model: TOFU pairing, SPKI pinning, HMAC, secret storage |
| [architecture/threat-model.md](architecture/threat-model.md) | Threat model and attack surface |
| [architecture/analisis-tecnico-profundo.pdf](architecture/analisis-tecnico-profundo.pdf) | Deep technical analysis (Gemini Deep Research, Spanish) — architecture, design trade-offs, data flows |

## Guides

| Document | Description |
|----------|-------------|
| [guides/tailscale-setup.md](guides/tailscale-setup.md) | Tailscale-specific setup, troubleshooting, and known limitations |

## Development

| Document | Description |
|----------|-------------|
| [development/TODO.md](development/TODO.md) | Known issues, pending work, and tech debt |
| [development/accessibility-service-plan.md](development/accessibility-service-plan.md) | Plan for Accessibility Service + Shizuku clipboard auto-read |
| [development/plan-auto-clipboard-detection.md](development/plan-auto-clipboard-detection.md) | Plan for automatic clipboard change detection on Android |
| [development/HANDOFF.md](development/HANDOFF.md) | Pipeline handoff notes (historical) |

## Development pipeline history

Phase summaries from the original 0–9 development pipeline (v0.1.0):

[phases/phase-1-summary.md](phases/phase-1-summary.md) →
[phase-2](phases/phase-2-summary.md) →
[phase-3](phases/phase-3-summary.md) →
[phase-4](phases/phase-4-summary.md) →
[phase-5](phases/phase-5-summary.md) →
[phase-6](phases/phase-6-summary.md) →
[phase-7](phases/phase-7-summary.md) →
[phase-8](phases/phase-8-summary.md) →
[phase-9](phases/phase-9-summary.md)
