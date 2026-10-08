# Camopanda

![A camouflage panda in a green forest](assets/camopanda-banner.jpeg)

Test how websites and APIs respond to requests that identify different browsers and devices.

Camopanda combines [Lightpanda](https://github.com/lightpanda-io/browser), a headless browser, with [Hudsucker](https://github.com/omjadas/hudsucker), a Rust HTTP proxy. Control the browser through Chrome DevTools Protocol (CDP), and set the outgoing `User-Agent` for your test case.

Use it to test desktop and mobile user-agent responses, check browser-specific server routing, and inspect requests from JavaScript `fetch` calls. The proxy replaces `User-Agent` and removes `Sec-Ch-Ua*` headers.

**These are request header profiles.** They do not emulate screen size, touch input, browser engines, browser APIs, or TLS fingerprints. Use real browsers and device emulation for visual layout and browser compatibility tests. JavaScript browser properties can still differ from the outgoing headers.

## Native CLI (macOS and Linux)

Install Lightpanda separately and make it available on `PATH`. The CLI embeds the Hudsucker proxy; Docker and OpenSSL are not required. Build with Rust 1.93 or later:

```sh
git clone https://github.com/paulmeller/camopanda.git
cd camopanda
cargo install --path proxy --locked --bin camopanda

camopanda fetch --dump markdown https://example.com
camopanda --user-agent "Your test user-agent" fetch --dump html https://httpbin.org/headers
camopanda serve --host 127.0.0.1 --port 9222
```

Connect agent-browser to the CLI's `serve` endpoint with the same CDP commands shown below.

Camopanda options go before the Lightpanda command. Subsequent arguments pass through to Lightpanda. `--http-proxy` and `--ca-cert` are reserved for Camopanda. `camopanda help fetch` displays Lightpanda's help. Output and ordinary exit codes pass through unchanged.

Each invocation starts an internal proxy on an available loopback port. Browser exit, Ctrl-C, and SIGTERM stop the browser and proxy. The CLI needs no separate proxy executable.

Certificates persist in `~/Library/Application Support/camopanda` on macOS, or `$XDG_DATA_HOME/camopanda` (default `~/.local/share/camopanda`) on Linux. The combined certificate and private key file has mode `0600`. This storage does not persist browser sessions.

Use `--state-dir PATH` to select another directory, and `--lightpanda PATH` or `LIGHTPANDA_BIN` to select a browser executable. `--user-agent` overrides `UPSTREAM_USER_AGENT`. Telemetry is disabled by default; `LIGHTPANDA_DISABLE_TELEMETRY` controls it. Native CLI settings come from arguments and environment variables; it does not read `.env`.

The CLI currently supports macOS and Linux. Lightpanda upgrades can change available commands and flags. Fetch was tested with the installed browser; not every upstream command has been verified through the wrapper.

## Docker quick start

Requirements: Docker with Docker Compose, and `agent-browser` installed on your computer.

On macOS, [container-compose](https://github.com/paulmeller/container-compose) provides a Compose workflow for Apple's container runtime. See its README for installation and usage.

Install the client with `npm install -g agent-browser`. Camopanda supplies the browser endpoint; the client does not need a local browser for these commands.

```sh
git clone https://github.com/paulmeller/camopanda.git
cd camopanda
docker compose up -d --build

agent-browser --session camopanda --cdp ws://127.0.0.1:9222/ open https://example.com
agent-browser --session camopanda --cdp ws://127.0.0.1:9222/ snapshot
```

The default profile sends a Mac Chrome 134 user-agent string. CDP listens on host loopback. The proxy has no published host port.

Stop the containers with `docker compose down`. Named volumes remain available for the next start.

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

An experimental CDP interception test checks whether separate connections can tag navigation, redirects, scripts, and fetch requests independently. It does not enable profile selection in the deployed proxy. With Node.js 22+ and Lightpanda installed, run:

```sh
node scripts/profile-spike.mjs
cargo build --manifest-path proxy/Cargo.toml --locked --bin camopanda
USE_PROXY=1 node scripts/profile-spike.mjs
```

The fixture runs locally. Direct Lightpanda requests carry the test marker; the proxy test checks that Camopanda strips it. This experiment does not select a per-session user agent. The session API uses separate processes to avoid depending on client interception hooks.

## Per-session user agents

Create a session for each browser or device request profile. Each session runs its own Lightpanda process and embedded proxy. This costs more resources than sharing one browser, but preserves client CDP commands, including request interception, and applies the profile to every proxied request.

The original endpoint on port 9222 keeps its existing behavior. The optional session service uses port 9223.

### Docker

Generate an API key and start the additional service:

```sh
export SESSION_API_KEY="$(openssl rand -hex 32)"
docker compose --profile sessions up -d --build
```

For a server deployment, set `SESSION_PUBLIC_URL` to the WebSocket origin clients can reach, such as `ws://<server-tailscale-ip>:9223`. Keep `CDP_BIND_IP` restricted to loopback or the server's private address. On Coolify, set `COMPOSE_PROFILES=sessions` and make that variable available at **both build time and runtime**. This ensures Coolify builds the optional image before starting it. Keep `SESSION_API_KEY` runtime-only, and configure the session variables below.

### Native

Install both executables alongside Lightpanda:

```sh
cargo install --path proxy --locked --bin camopanda --bin camopanda-gateway
export SESSION_API_KEY="$(openssl rand -hex 32)"
camopanda-gateway
```

The gateway finds `camopanda` beside its own executable, or at `CAMOPANDA_BIN`. Both native commands support macOS and Linux.

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

Deletion closes the CDP connection and stops the browser and proxy. Idle sessions expire automatically. Activity means CDP messages in either direction; inspection requests do not extend the timeout. Disconnected sessions can reconnect before expiry, but browser page state and cookies are not guaranteed to survive reconnection. All sessions are lost on gateway restart. Its persistent volume stores certificates, not session records.

User-agent values must be nonempty valid HTTP header values, at most 512 bytes. The gateway accepts only a `user_agent` field. It reserves capacity before browser startup; excess sessions receive HTTP 429. Startup failures receive HTTP 503. Internal `X-Camopanda-*` headers and `Sec-Ch-Ua*` headers are stripped before requests leave the proxy.

| Variable | Default | Purpose |
|---|---|---|
| `SESSION_API_KEY` | Required | Bearer key for create, inspect, and delete; at least 32 bytes. |
| `SESSION_PUBLIC_URL` | `ws://127.0.0.1:9223` | WebSocket origin used in returned client URLs. |
| `SESSION_MAX_SESSIONS` | `4` | Maximum starting and live sessions, from 1 to 32. |
| `SESSION_IDLE_SECS` | `300` | Idle timeout, from 1 to 86400 seconds; sweep runs each second. |
| `SESSION_PORT` | `9223` | Published Docker host port; update the public URL if changed. |
| `SESSION_BIND` | `127.0.0.1:9223` native | Native listening address; Docker uses `0.0.0.0:9223`. |
| `SESSION_STATE_DIR` | User application-data directory under `camopanda/sessions` native; `/state` Docker | Certificate storage shared by the session wrappers. |
| `CAMOPANDA_BIN` | Executable beside the gateway | Native wrapper location. |

The gateway provides an unauthenticated `/health` probe. CDP messages are limited to 16 MiB. Keep the service on a restricted network even with authentication: clients can access destinations reachable from the server. Per-request profile changes inside one session are not supported; create another session to change the user agent.

Run the end-to-end session checks locally with Node.js 22+, Lightpanda, and `curl` installed:

```sh
cargo build --manifest-path proxy/Cargo.toml --locked --bins
node scripts/session-smoke.mjs
TEST_AGENT_BROWSER=1 node scripts/session-smoke.mjs
```

The smoke test checks concurrent profiles, redirect/script/fetch headers, client interception, authentication, capacity, reconnection, deletion, idle expiry, and process cleanup. The optional check also runs agent-browser.

To inspect outgoing headers with a public echo service:

```sh
agent-browser --session camopanda --cdp ws://127.0.0.1:9222/ open https://httpbin.org/headers
agent-browser --session camopanda --cdp ws://127.0.0.1:9222/ snapshot
```

The response should show your configured `User-Agent` and no `Sec-Ch-Ua*` headers.

## How it works

```text
Agent / agent-browser
        | CDP
        v
    Lightpanda
        | HTTP proxy requests
        v
    Hudsucker
        | rewritten headers
        v
    Website / API
```

1. The client sends a browser command to Lightpanda through CDP.
2. Lightpanda sends website requests, including JavaScript fetch requests, through Hudsucker.
3. Hudsucker replaces `User-Agent` and removes headers whose names start with `Sec-Ch-Ua`.
4. Hudsucker sends the request to the website and returns the response to Lightpanda.
5. Lightpanda processes the page and executes JavaScript.
6. The client requests page data, such as a snapshot, through CDP.

For HTTPS, Hudsucker terminates the browser's TLS connection and makes a separate TLS connection to the website. It verifies the website's certificate.

At first start, the proxy creates a certificate authority. The `proxy-state` volume stores its certificate and private key. The `proxy-trust` volume shares only the public certificate with Lightpanda through a read-only mount. Keep the private key out of source control.

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
| `http://proxy:8080` | Internal proxy address inside the Compose network. |

The HTTP discovery endpoint does not fetch websites. Camopanda does not provide a REST fetch endpoint.

For remote access, use the server address in the WebSocket URL. Discovery can advertise a loopback address; remote clients should use the explicit server URL.

## Deploy with Coolify

Create a Git-backed Docker Compose application and select `/docker-compose.yaml`. Both images build from this repository. Preserve the named volumes between deployments.

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

Connected clients can instruct the browser to access destinations reachable from the server, including internal services. Use a dedicated test environment and trusted clients. The proxy does not log request URLs, but upstream components can produce their own logs.

## Upstream projects and licensing

Camopanda packages separate Lightpanda and Hudsucker components. It is an independent project.

- [Lightpanda](https://github.com/lightpanda-io/browser/blob/main/LICENSING.md) uses AGPL-3.0-only.
- [Hudsucker](https://github.com/omjadas/hudsucker) is available under MIT or Apache-2.0.

Camopanda's original code is licensed under [MIT](LICENSE). Lightpanda remains licensed under AGPL-3.0-only. Other dependencies retain their respective licenses; Camopanda's MIT license does not replace their terms.

## Validation and known limits

Live checks passed for navigation, snapshots, HTTPS header rewriting, and direct Tailscale access on a Coolify deployment. External-site compatibility depends on the site and has not been tested comprehensively.

- Login and cookie persistence across browser restarts remain unverified. The persistent volumes store proxy certificates, not a browser profile.
- Automatic handoff to a human for login is not implemented.
- Lightpanda uses a mutable `nightly` image. Pin a tested image digest for reproducible deployments.
- The proxy healthcheck checks its listener. It does not check external DNS or end-to-end navigation.

Run the proxy checks locally:

```sh
cargo test --manifest-path proxy/Cargo.toml --locked
cargo clippy --manifest-path proxy/Cargo.toml --locked --all-targets -- -D warnings
cargo fmt --manifest-path proxy/Cargo.toml --check
```
