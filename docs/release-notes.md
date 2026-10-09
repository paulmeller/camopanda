Camopanda 0.2.0 replaces the HTTPS rewriting proxy with a pinned, patched Lightpanda browser for both Docker and the native CLI.

- One default browser container; no Hudsucker, TLS interception or certificate volumes.
- Bundled CLI/browser archives for Linux amd64/arm64 and macOS arm64.
- Existing CDP port 9222 and optional authenticated session API on port 9223.
- Immutable session user agents, client-hint stripping, process isolation and lifecycle bounds.
- Patched Lightpanda source, patch, pins and checksums accompany the release.

See README.md and docs/migration.md for installation, supported platforms and migration. These are HTTP request profiles, not browser-engine/device emulation. Camopanda code is MIT; the bundled patched Lightpanda retains its upstream AGPL license.
