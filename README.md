# Camopanda

![A camouflage panda in a green forest](assets/camopanda-banner.jpeg)

Test how websites and APIs respond to requests that identify different browsers and devices.

Camopanda packages a pinned, patched [Lightpanda](https://github.com/lightpanda-io/browser) headless browser with direct CLI and Docker parity. Control the browser through Chrome DevTools Protocol (CDP), and set the outgoing `User-Agent` for your test cases. Request headers are controlled inside the browser engine; no rewriting proxy, TLS interception or certificate setup is required.

Use it to test desktop and mobile user-agent responses, check browser-specific server routing, and inspect requests from JavaScript `fetch` calls. The browser sends your selected `User-Agent` and removes `Sec-Ch-Ua*` client-hint headers.

**These are request header profiles.** They do not emulate screen size, touch input, browser engines, browser APIs, or TLS fingerprints. Use real browsers and device emulation for visual layout and browser compatibility tests. JavaScript browser properties can still differ from outgoing headers.

## Native CLI

Download the [release bundle](https://github.com/paulmeller/camopanda/releases), or use the installer:

```sh
curl --fail --location -o install-camopanda.sh \
  https://raw.githubusercontent.com/paulmeller/camopanda/v0.2.0/scripts/install.sh
sh install-camopanda.sh
export PATH="$HOME/.local/bin:$PATH"

camopanda version
camopanda fetch https://example.com --dump markdown
UPSTREAM_USER_AGENT="Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15" \
  camopanda fetch https://httpbin.org/headers --dump html
camopanda serve --host 127.0.0.1 --port 9222
```

Supported platforms are macOS 14+ on Apple Silicon and glibc Linux amd64/arm64 (tested on Ubuntu 24.04). Bundles provide `camopanda` (with `lightpanda` symlinked).

Options and arguments match Lightpanda 1:1. Set `UPSTREAM_USER_AGENT` in the environment, or pass `--user-agent <STRING>` with `--camopanda-header-profiles`.

Telemetry is disabled by default; `LIGHTPANDA_DISABLE_TELEMETRY` controls it.

### Build from source

Python 3, Git, Make, Clang, Curl, XZ, and Rust 1.93+ (for upstream FFI dependencies) are required. The build downloads checksum-verified, pinned Zig and V8 tools:

```sh
git clone https://github.com/paulmeller/camopanda.git
cd camopanda
python3 scripts/build-browser.py --test
dist/camopanda fetch https://example.com --dump markdown
```

See [browser build and upgrade notes](browser/README.md).

## Docker quick start

Requirements: Docker Compose and a CDP client (e.g. `npm install -g agent-browser`).

```sh
git clone https://github.com/paulmeller/camopanda.git
cd camopanda
docker compose pull
docker compose up -d --no-build

agent-browser --session camopanda --cdp ws://127.0.0.1:9222/ open https://example.com
agent-browser --session camopanda --cdp ws://127.0.0.1:9222/ snapshot
```

The versioned image contains the patched browser. To build locally instead, run:
```sh
docker compose -f docker-compose.yaml -f docker-compose.build.yaml up -d --build
```

The default stack is a single lightweight container with no proxy or certificate volumes. `CAMOPANDA_VERSION` selects the image version (default: `0.2.0`).

The default profile sends a Mac Chrome 134 user-agent string. CDP binds to host loopback. Stop with `docker compose down`.

Upgrading from the old proxy backend? Follow the [migration instructions](docs/migration.md).

## Test browser and device profiles

Set `UPSTREAM_USER_AGENT` in a local `.env` file. For example, to identify requests as Safari on an iPhone:

```dotenv
UPSTREAM_USER_AGENT="Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1"
```

For a desktop Firefox profile:

```dotenv
UPSTREAM_USER_AGENT="Mozilla/5.0 (X11; Linux x86_64; rv:128.0) Gecko/20100101 Firefox/128.0"
```

Apply the configuration and restart the container:

```sh
docker compose up -d --force-recreate
```

To inspect outgoing headers with a public echo service:

```sh
agent-browser --session camopanda --cdp ws://127.0.0.1:9222/ open https://httpbin.org/headers
agent-browser --session camopanda --cdp ws://127.0.0.1:9222/ snapshot
```

The response shows your configured `User-Agent` and no `Sec-Ch-Ua*` headers.

## Configuration

| Variable | Default | Purpose |
|---|---|---|
| `UPSTREAM_USER_AGENT` | Mac Chrome 134 string | Sets the outgoing user-agent header. |
| `CDP_PORT` | `9222` | Sets the published host port. |
| `CDP_BIND_IP` | `127.0.0.1` | Sets the host address for CDP. |
| `LIGHTPANDA_DISABLE_TELEMETRY` | `true` | Disables Lightpanda usage telemetry. |

Copy `.env.example` to `.env` to customize these settings.

## Remote access

Run Docker Compose on your server.

For direct access through Tailscale, set `CDP_BIND_IP` to the server's Tailscale IPv4 address. Connect your computer to the same tailnet, then run:

```sh
agent-browser --session camopanda --cdp ws://<server-tailscale-ip>:9222/ open https://example.com
agent-browser --session camopanda --cdp ws://<server-tailscale-ip>:9222/ snapshot
```

Alternatively, keep the default loopback binding and create an SSH tunnel:

```sh
ssh -N -L 9222:127.0.0.1:9222 user@your-server
```

Then connect to `ws://127.0.0.1:9222/` on your computer.

CDP has no built-in application authentication. Restrict access with Tailscale access rules, a firewall, or an SSH tunnel. Do not expose port 9222 directly to the public internet.

## Upstream projects and licensing

Camopanda packages a patched Lightpanda browser. It is an independent project.

- [Lightpanda](https://github.com/lightpanda-io/browser/blob/main/LICENSING.md) uses AGPL-3.0-only.

Camopanda's original code and patches are licensed under [MIT](LICENSE). The patched Lightpanda remains licensed under AGPL-3.0-only. Each release includes its corresponding patched source, patch and build pins; Docker images also contain the source archive at `/usr/local/share/camopanda/lightpanda-source.tar.gz`.
