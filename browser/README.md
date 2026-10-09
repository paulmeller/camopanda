# Patched Lightpanda backend

`pins.json` locks the source revision, Zig version and V8 archive checksums.
`lightpanda.patch` is applied to that exact source by `scripts/build-browser.py`.
The patch remains subject to the upstream source license; Camopanda's MIT license
covers its original wrapper/gateway code, not Lightpanda.

The opt-in `--camopanda-header-profiles` flag accepts printable browser user agents
and strips `Sec-Ch-Ua*` and `X-Camopanda-*` headers. The additional
`--camopanda-lock-user-agent` flag preserves the startup user agent even if a CDP
client requests another one. Request header overrides cannot defeat either rule.
The patch covers HTTP requests and JavaScript WebSocket handshakes. It does not
change TLS fingerprints, browser engines, viewport/touch behavior or JavaScript
client-hint properties. Processes remain isolated per Camopanda session.

`lightpanda version --camopanda-capabilities` emits the capability marker checked
by the CLI and gateway. Stock builds do not satisfy that check. The default mode
without these flags retains upstream identity restrictions and headers.

Build and export the browser and corresponding patched source:

```sh
python3 scripts/build-browser.py --test
```

The build downloads and checksum-verifies the pinned Zig and V8 archives. Source
dependencies are pinned by upstream's checked-in manifests. It outputs the browser,
licenses, pins and `lightpanda-source.tar.gz` in `dist`. Native builds require Git,
Python 3, Make, Rust, Clang, Curl and XZ; Docker includes these build tools.

For an upstream upgrade, update the pins and patch on a branch. Require clean
patch application, builds on all supported platforms, the complete upstream test
suite without opt-in flags, and both Camopanda header/lifecycle fixtures. Test
fixture behavior, not just patch application: upstream can introduce request paths
that do not call the patched header functions. Do not publish an upgrade if any
platform or invariant fails.

Release archives and Docker images retain Lightpanda's license. The release's
source archive contains the patched tree, including upstream build/dependency
manifests. The Docker image also includes this archive at
`/usr/local/share/camopanda/lightpanda-source.tar.gz`; it can be extracted with
`docker cp`. Camopanda's release includes the patch and pins to reproduce it.
