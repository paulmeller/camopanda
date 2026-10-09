# Camopanda

![A camouflage panda in a green forest](assets/camopanda-banner.jpeg)

Test how websites and APIs respond to requests that identify different browsers and devices.

Camopanda packages a pinned, patched [Lightpanda](https://github.com/lightpanda-io/browser) browser with a native CLI and an optional authenticated session API. Control the browser through Chrome DevTools Protocol (CDP), and set the outgoing `User-Agent` for your test case. Request headers are controlled inside the browser; no rewriting proxy, TLS interception or certificate setup is required.

Use it to test desktop and mobile user-agent responses, check browser-specific server routing, and inspect requests from JavaScript `fetch` calls. The browser sends your selected `User-Agent` and removes `Sec-Ch-Ua*` headers.

**These are request header profiles.** They do not emulate screen size, touch input, browser engines, browser APIs, or TLS fingerprints. Use real browsers and device emulation for visual layout and browser compatibility tests. JavaScript browser properties can still differ from the outgoing headers.

## Native CLI

Download the [release bundle](https://github.com/paulmeller/camopanda/releases), or use the checksum-verifying installer:

```sh
curl --fail --location -o install-camopanda.sh \
  https://raw.githubusercontent.com/paulmeller/camopanda/v0.2.0/scripts/install.sh
sh install-camopanda.sh
export PATH="$HOME/.local/bin:$PATH"

camopanda --version
camopanda fetch --dump markdown https://example.com
camopanda --user-agent "Your test user-agent" fetch --dump html https://httpbin.org/headers
camopanda serve --host 127.0.0.1 --port 9222
```

Bundles contain `camopanda`, `camopanda-gateway` and the patched `lightpanda` executable. Supported platforms are macOS 14+ on Apple Silicon and glibc Linux amd64/arm64 (built on Debian 12). Docker supports Linux amd64/arm64. Windows and Intel macOS bundles are not supplied.

Camopanda options go before the Lightpanda command. Subsequent arguments pass through. Output and ordinary exit codes pass through unchanged; Ctrl-C and SIGTERM stop and reap the browser. `camopanda help fetch` displays upstream help. `camopanda --version` reports Camopanda; `lightpanda version` reports the browser.

The CLI first looks for its bundled `lightpanda` sibling, then on `PATH`. Use `--lightpanda PATH` or `LIGHTPANDA_BIN` to select another compatible patched build. Camopanda checks its patch capability and refuses stock Lightpanda. `--user-agent` overrides `UPSTREAM_USER_AGENT`; profile flags and user-agent flags after the command are reserved. `--http-proxy` and `--ca-cert` are ordinary upstream options for an explicitly configured external proxy or private CA.

The CLI creates no certificates or persistent browser profile. The old `--state-dir` option is accepted with a deprecation message and ignored. Telemetry is disabled by default; `LIGHTPANDA_DISABLE_TELEMETRY` controls it. Native settings come from arguments and environment variables, not `.env`.

### Build from source

Rust 1.93+, Python 3, Git, Make, Clang, Curl and XZ are required. The browser build downloads checksum-verified, pinned Zig and V8 tools; it can take several minutes.

```sh
git clone https://github.com/paulmeller/camopanda.git
cd camopanda
python3 scripts/build-browser.py --test
cargo build --manifest-path cli/Cargo.toml --locked --release --bins
cp dist/lightpanda cli/target/release/lightpanda
cli/target/release/camopanda fetch --dump markdown https://example.com
```

See [browser build and upgrade notes](browser/README.md). Not every upstream command has been verified through the wrapper.

## Docker quick start

Requirements: Docker Compose and a CDP client. Install agent-browser with `npm install -g agent-browser`.

```sh
git clone https://github.com/paulmeller/camopanda.git
cd camopanda
docker compose pull
docker compose up -d --no-build

agent-browser --session camopanda --cdp ws://127.0.0.1:9222/ open https://example.com
agent-browser --session camopanda --cdp ws://127.0.0.1:9222/ snapshot
```

The versioned image contains the same patched browser as the CLI bundle. To build locally instead, run `docker compose up -d --build`. The default stack is one browser container, with no proxy or certificate volumes. `CAMOPANDA_VERSION` selects the image version; the default is `0.2.0`.

The default profile sends a Mac Chrome 134 user-agent string. CDP binds to host loopback. Stop with `docker compose down`.

On macOS, [container-compose](https://github.com/paulmeller/container-compose) provides a Compose workflow for Apple's container runtime. See its README for installation and usage.

Upgrading from the old proxy backend? Follow the [migration and rollback instructions](docs/migration.md).

## Test browser and device profiles

Set `UPSTREAM_USER_AGENT` in a local `.env` file. For example, this profile identifies requests as Safari on an iPhone:

```dotenv
UPSTREAM_USER_AGENT="Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1"
```

For a desktop Firefox profile, use:

```dotenv
UPSTREAM_USER_AGENT="Mozilla/5.0 (X11; Linux x86_64; rv:128.0) Gecko/20100101 Firefox/128.0"
```

These are example header values, not claims about current browser releases. Use the exact value required by your test.

Apply the configuration, then repeat your navigation or fetch test:

```sh
docker compose up -d --force-recreate
```

All requests through the default CDP endpoint use the configured profile. That endpoint shares one profile across clients. The optional session API below provides isolated profiles.

## Per-session user agents

Create a session for each browser or device request profile. Each session runs its own patched Lightpanda process. This costs more resources than sharing one browser, but preserves client CDP commands, including request interception, and applies the profile to HTTP requests and JavaScript WebSocket handshakes.

The original endpoint on port 9222 keeps its existing behavior. The optional session service uses port 9223.

### Docker

Generate an API key and start the additional service:

```sh
export SESSION_API_KEY="$(openssl rand -hex 32)"
docker compose --profile sessions pull
docker compose --profile sessions up -d --no-build
```

For a server deployment, set `SESSION_PUBLIC_URL` to the WebSocket origin clients can reach, such as `ws://<server-tailscale-ip>:9223`. Keep `CDP_BIND_IP` restricted to loopback or the server's private address. Configure the session variables below.

### Native

The release bundle includes the gateway and patched browser:

```sh
export SESSION_API_KEY="$(openssl rand -hex 32)"
camopanda-gateway
```

The gateway finds the patched `lightpanda` beside its own executable, or at `LIGHTPANDA_BIN`. It launches the browser directly; no wrapper or proxy process is needed per session.

### Create and use a session

```sh
curl --fail http://127.0.0.1:9223/v1/sessions \
  -H "Authorization: Bearer $SESSION_API_KEY" \
  -H 'Content-Type: application/json' \
  --data '{"user_agent":"Your browser or device test user-agent"}'
```

The JSON response contains `id`, `user_agent`, `cdp_url`, and `idle_timeout_secs`. Pass the returned `cdp_url` to your CDP client:

```sh
agent-browser --session profile-test --cdp '<returned-cdp-url>' open https://example.com
agent-browser --session profile-test --cdp '<returned-cdp-url>' snapshot
```

The CDP URL contains a session-specific access token. Treat the complete URL as a credential. One client can connect to a session at a time. A second connection receives HTTP 409. Create another session for another concurrent client.

Inspect or delete a session with the administrator API key:

```sh
curl --fail -H "Authorization: Bearer $SESSION_API_KEY" \
  http://127.0.0.1:9223/v1/sessions/<id>
curl --fail -X DELETE -H "Authorization: Bearer $SESSION_API_KEY" \
  http://127.0.0.1:9223/v1/sessions/<id>
```

Deletion closes the CDP connection and stops the browser. Idle sessions expire automatically. Activity means CDP messages in either direction; inspection requests do not extend the timeout. Disconnected sessions can reconnect before expiry, but browser page state and cookies are not guaranteed to survive reconnection. All sessions are lost on gateway restart. The gateway needs no persistent volume.

User-agent values must be nonempty printable ASCII, at most 512 bytes. CDP clients cannot change the configured session user agent. The gateway accepts only a `user_agent` field. It reserves capacity before browser startup; excess sessions receive HTTP 429. Startup failures receive HTTP 503. Internal `X-Camopanda-*` headers and `Sec-Ch-Ua*` headers are stripped before requests leave the browser.

| Variable | Default | Purpose |
|---|---|---|
| `SESSION_API_KEY` | Required | Bearer key for create, inspect, and delete; at least 32 bytes. |
| `SESSION_PUBLIC_URL` | `ws://127.0.0.1:9223` | WebSocket origin used in returned client URLs. |
| `SESSION_MAX_SESSIONS` | `4` | Maximum starting and live sessions, from 1 to 32. |
| `SESSION_IDLE_SECS` | `300` | Idle timeout, from 1 to 86400 seconds; sweep runs each second. |
| `SESSION_PORT` | `9223` | Published Docker host port; update the public URL if changed. |
| `SESSION_BIND` | `127.0.0.1:9223` native | Native listening address; Docker uses `0.0.0.0:9223`. |
| `LIGHTPANDA_BIN` | Patched browser beside the gateway, then on `PATH` | Browser executable location. |

The gateway provides an unauthenticated `/health` probe. CDP messages are limited to 16 MiB. Keep the service on a restricted network even with authentication: clients can access destinations reachable from the server. Per-request profile changes inside one session are not supported; create another session to change the user agent.

Run the end-to-end session checks locally with Node.js 22+, the patched Lightpanda, and `curl` installed:

```sh
cargo build --manifest-path cli/Cargo.toml --locked --bins
export LIGHTPANDA_BIN="$PWD/dist/lightpanda"
node scripts/header-smoke.mjs
LOCK_PROFILE=1 node scripts/header-smoke.mjs
node scripts/session-smoke.mjs
TEST_AGENT_BROWSER=1 node scripts/session-smoke.mjs
```

The smoke test checks concurrent profiles, redirect/script/fetch headers, client interception, authentication, capacity, reconnection, deletion, idle expiry, and process cleanup. The optional check also runs agent-browser.

To inspect outgoing headers with a public echo service:

```sh
agent-browser --session camopanda --cdp ws://127.0.0.1:9222/ open https://httpbin.org/headers
agent-browser --session camopanda --cdp ws://127.0.0.1:9222/ snapshot
```

The response should show your configured `User-Agent` and no `Sec-Ch-Ua*` headers. Header tests also cover WS/WSS handshakes. JavaScript client-hint properties can still identify Lightpanda.

## How it works

```text
Client / agent-browser
        | CDP
        v
Patched Lightpanda
        | selected request headers, direct HTTP/TLS
        v
    Website / API
```

The CLI starts the patched browser with the selected profile. For sessions, the gateway authenticates creation, reserves capacity and starts an isolated browser. It relays CDP messages without modifying client commands. Lightpanda applies the profile at request construction, handles redirects, executes JavaScript and verifies destination TLS certificates normally.

There is no intermediate TLS connection or generated certificate authority. Profile headers take priority over script and CDP request-header overrides. The patch is opt-in; its normal mode preserves upstream behavior. See the [patch and pinned build inputs](browser/README.md).

## Configuration

| Variable | Default | Purpose |
|---|---|---|
| `UPSTREAM_USER_AGENT` | Mac Chrome 134 string | Sets the outgoing user-agent header. |
| `CDP_PORT` | `9222` | Sets the published host port. |
| `CDP_BIND_IP` | `127.0.0.1` | Sets the host address for CDP. |
| `LIGHTPANDA_DISABLE_TELEMETRY` | `true` | Disables Lightpanda usage telemetry. |

Copy `.env.example` to `.env` to customize these settings. Camopanda disables upstream telemetry by default. See [Lightpanda's telemetry documentation](https://github.com/lightpanda-io/browser#telemetry).

## Endpoints

| Endpoint | Purpose |
|---|---|
| `ws://127.0.0.1:9222/` | Browser control through CDP. |
| `http://127.0.0.1:9222/json/version` | Browser and protocol information. |

The HTTP discovery endpoint does not fetch websites. Camopanda does not provide a REST fetch endpoint.

For remote access, use the server address in the WebSocket URL. Discovery can advertise a loopback address; remote clients should use the explicit server URL.

## Remote access

Run the Docker Compose stack on your server. Preserve the named volumes between deployments.

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

CDP has no application authentication. Restrict access with Tailscale access rules or an SSH tunnel. Do not publish this port to the public internet.

Connected clients can instruct the browser to access destinations reachable from the server, including internal services. Use a dedicated test environment and trusted clients. Upstream browser logs can contain request information.

## Upstream projects and licensing

Camopanda packages a patched Lightpanda browser. It is an independent project.

- [Lightpanda](https://github.com/lightpanda-io/browser/blob/main/LICENSING.md) uses AGPL-3.0-only.

Camopanda's original code is licensed under [MIT](LICENSE). The patched Lightpanda remains licensed under AGPL-3.0-only. Each release includes its corresponding patched source, patch and build pins; Docker images also contain the source archive at `/usr/local/share/camopanda/lightpanda-source.tar.gz`. Other dependencies retain their respective licenses; Camopanda's MIT license does not replace their terms.

## Validation and known limits

Live checks passed for navigation, snapshots, HTTPS header profiles, and direct Tailscale access on a remote Docker deployment. External-site compatibility depends on the site and has not been tested comprehensively.

- Login and cookie persistence across browser restarts remain unverified. No persistent browser profile is configured.
- Automatic handoff to a human for login is not implemented.
- The browser source revision, Zig version and V8 archives are pinned. Upgrades require cross-platform builds and integration checks.
- The gateway healthcheck checks its listener. It does not check external DNS or end-to-end navigation.

Run the CLI and gateway checks locally:

```sh
cargo test --manifest-path cli/Cargo.toml --locked
cargo clippy --manifest-path cli/Cargo.toml --locked --all-targets -- -D warnings
cargo fmt --manifest-path cli/Cargo.toml --check
```
