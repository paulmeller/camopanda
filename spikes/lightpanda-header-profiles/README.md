# Direct Lightpanda header-profile spike

This experiment replaces proxy header rewriting with a small, opt-in Lightpanda
patch. It does not change Camopanda's production binaries, Compose stack or
deployment. The patch modifies AGPL-licensed upstream source; Camopanda's MIT
license does not replace that source's license.

## Reproduce

Requires macOS or Linux, Zig 0.17.0, Rust, Make, Git, OpenSSL and Node.js 22+.
The tested upstream revision is `d123cf711b8c0d80f9d0bb7a4117130a195d3105`.

```sh
git clone https://github.com/lightpanda-io/browser.git lightpanda-test
cd lightpanda-test
git checkout d123cf711b8c0d80f9d0bb7a4117130a195d3105
git apply --check /path/to/camopanda/spikes/lightpanda-header-profiles/lightpanda.patch
git apply /path/to/camopanda/spikes/lightpanda-header-profiles/lightpanda.patch
make download-v8
zig build -Doptimize=fast -j4
env -u CAMOPANDA_TEST_HEADERS make test ZIGFLAGS=-j4
cd /path/to/camopanda
LIGHTPANDA_BIN=/path/to/lightpanda-test/zig-out/bin/lightpanda \
  node spikes/lightpanda-header-profiles/check.mjs
```

Set `CAMOPANDA_TEST_HEADERS=1` on the patched browser to enable the experiment.
Without this exact setting, the upstream Mozilla restriction and client-hint
headers remain in place. The fixture sets the variable on its own child
processes, creates an ephemeral HTTPS certificate, and cleans up its processes
and files.

The mode accepts printable ASCII user agents containing Mozilla, strips all
`Sec-Ch-Ua*` and `X-Camopanda-*` headers, and gives the configured user agent fixed
priority over request-header overrides. Existing CLI and CDP user-agent controls
select the profile. It does not implement browser-engine or device emulation.

The fixture checks two concurrent processes, CLI and CDP profiles, fresh targets,
HTTP and HTTPS navigation, redirects, scripts, JavaScript fetch, and conflicting
headers supplied by client-owned CDP Fetch interception.

## Release maintenance

Keep the patch outside upstream and pin each adopted upstream revision. For each
upgrade, require clean patch application, a fresh build, upstream tests with the
mode disabled, and the direct-header fixture with the mode enabled. A clean
application alone does not prove that new request paths honor the profile.

Keep process-per-session isolation initially: the current CDP override lives on
the browser HTTP client, and this spike does not prove context-level isolation.
Shipping this backend would also require build/release packaging and gateway
integration; the existing gateway still launches the proxy-backed CLI.

## Results

Tested on macOS arm64 with Zig 0.17.0:

- Stock installed Lightpanda rejected the fixture's Mozilla user agent with
  `err=Reserved`, as expected.
- Patched release build succeeded; the opt-in fixture passed 32 direct HTTP/HTTPS
  requests across two concurrent processes, CLI and CDP profiles, fresh targets,
  redirects, scripts, fetch and attempted interception overrides.
- With the mode disabled, all 1,731 upstream tests passed; five were skipped.
  The patched binary still rejected Mozilla with `err=Reserved`.
- Agent-browser directly connected to the patched browser and passed public HTTPS
  navigation and fetch header assertions against httpbin.org, without a proxy or
  custom CA.
- Upstream formatting checks passed. The patch applied cleanly to a fresh
  checkout of the pinned revision.

This establishes feasibility for replacing the proxy for the tested HTTP request
paths. Linux/Windows builds, WebSocket handshake headers, uncommon request paths,
shared-process context isolation, memory/latency comparisons and upgrades to a
second upstream revision were not verified. JavaScript client-hint properties
still identify Lightpanda; the patch does not emulate those properties.

Before adopting this backend, move the experimental environment switch into a
proper startup configuration, package pinned AGPL-licensed browser builds with
corresponding source, integrate the gateway without the proxy-backed wrapper,
and test Linux container builds and lifecycle cleanup. Preserve the current
proxy backend until the new backend passes that integration suite.
