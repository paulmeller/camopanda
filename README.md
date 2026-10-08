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

All requests through this stack use the configured profile. The profile is shared across clients; it is not a per-session setting.

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

Each dependency retains its own license. A license for Camopanda's own code must be selected before publication.

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
