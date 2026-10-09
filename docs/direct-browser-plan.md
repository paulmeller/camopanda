# Direct patched Lightpanda replacement

Approved scope: replace Hudsucker for both native CLI and Docker; keep commands,
port 9222, authenticated session API on 9223, immutable session profiles, lifecycle
bounds and cleanup. Publish pinned browser builds, CLI archives and Docker images
for Linux amd64/arm64 and macOS arm64, with corresponding patched source. Preserve
previous releases for rollback. Adopt upgrades only after build, upstream tests
and header/lifecycle tests pass.

## Completion gates

- [ ] Maintained opt-in startup patch; capability marker; locked profiles;
      validation; HTTP/HTTPS and WS/WSS header paths.
- [ ] Pinned source, Zig and V8; reproducible build scripts and source distribution.
- [ ] CLI directly launches compatible browser; version; arguments/output/exits;
      SIGINT/SIGTERM; deprecated state-dir creates no files.
- [ ] Gateway directly launches browser; authentication/capacity/expiry/reconnect/
      cleanup and capability URLs unchanged.
- [ ] Docker default single browser service and optional sessions; no proxy or CA.
- [ ] Native macOS arm64 and Linux amd64/arm64 builds and integration checks.
- [ ] Header fixtures, upstream tests, Rust tests/lints, Docker smoke; resource
      comparison against previous backend.
- [ ] Release binaries/checksums/source/images and installation/migration docs.
- [ ] Merge/push, deploy, verify both live endpoints, clean test sessions; rollback
      documented and previous immutable artifacts available.

## Current progress

Feature branch feat/direct-lightpanda includes initial direct CLI/gateway refactor.
13 Rust tests pass locally. Maintained patch is in browser/lightpanda.patch.
Upstream checkout /tmp/camopanda-upstream-review at d123cf7, Zig toolchain
/tmp/zig-aarch64-macos-0.17.0. Build and full upstream tests running; packaging,
Docker and release/deployment work remains. Spike branch preserves prior results.
