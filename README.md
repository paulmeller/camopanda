# Camopanda

Lightpanda with an HTTPS header-rewriting proxy and a CDP endpoint for browser agents.

Hudsucker sets a configurable outgoing User-Agent and removes `Sec-Ch-Ua*` headers. This changes HTTP headers; it does not reproduce Chrome rendering, browser APIs or its TLS fingerprint.

## Run

```sh
docker compose up -d --build
agent-browser --cdp 9222 open https://example.com
agent-browser --cdp 9222 snapshot -i
```

Set `UPSTREAM_USER_AGENT` in `.env` to override the synthetic Mac Chrome 134 string. Set `CDP_PORT` to change the host port. Restart/recreate the stack after configuration changes.

Lightpanda connects internally to `proxy:8080`. The proxy generates its CA in the persistent `proxy-state` volume and publishes only the public certificate into `proxy-trust`, which Lightpanda reads. Upstream TLS certificates are verified. CDP is bound to host loopback; the proxy has no published port. Keep the private CA volume out of source control.

## Coolify

Create a Git-backed Docker Compose application using this private repository and `/docker-compose.yml`. Both images build from this repository. Keep the named volumes persistent and leave CDP on loopback.

From your Mac, tunnel to the server:

```sh
ssh -N -L 9222:127.0.0.1:9222 user@your-server
```

Then use the same `agent-browser --cdp 9222` commands.

## Validation and limits

```sh
cargo test --manifest-path proxy/Cargo.toml --locked
cargo clippy --manifest-path proxy/Cargo.toml --locked -- -D warnings
cargo fmt --manifest-path proxy/Cargo.toml --check
```

The source stack was verified with Apple containers on 2026-10-05: agent-browser navigation and snapshots succeeded, and an HTTPS header echo returned HTTP 200 with the configured UA and no `Sec-Ch-Ua*`. A broken Apple NAT network initially prevented routing, DNS and port forwarding; rebuilding that project network repaired it.

This standalone Docker/Coolify packaging has not yet been deployed. Shared named trust volumes target Docker; the Apple-container spike used a bind mount instead. Lightpanda's `nightly` image is mutable. Pin a tested image digest when reproducible deployments are required. The healthcheck tests the proxy listener, not external DNS or end-to-end navigation.
