Camopanda 0.2.0 replaces the HTTPS rewriting proxy with a pinned, patched Lightpanda browser for Docker and direct CLI usage.

- Single standalone browser container; no Hudsucker proxy, TLS interception, or certificate volumes.
- Parity with upstream Lightpanda: permits browser user agents and strips `Sec-Ch-Ua*` client hints.
- Direct CDP on port 9222.
- Native binaries for Linux amd64/arm64 and macOS arm64.
- Patched Lightpanda source, patch, pins, and checksums accompany the release.

See README.md and docs/migration.md for installation, supported platforms, and migration. These are HTTP request profiles, not browser-engine/device emulation. Camopanda code is MIT; the bundled patched Lightpanda retains its upstream AGPL license.
