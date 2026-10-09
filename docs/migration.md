# Upgrade from the proxy backend

Camopanda 0.2 replaces the rewriting proxy with a patched browser. Your configured
`UPSTREAM_USER_AGENT`, `CDP_BIND_IP`, `CDP_PORT` and session settings keep the same
meaning. Browser request profiles remain distinct from browser/device emulation.

## Docker

Save your previous Compose file and its configuration. Stop the old stack with
its old Compose file before starting the new one: its services were named
`proxy`, `lightpanda` and `sessions`; the new ones are `browser` and `sessions`.
Alternatively use `--remove-orphans` with the updated file after pulling/building
it. Keep the existing port bindings restricted to loopback or a private network.

```sh
docker compose --profile sessions pull
docker compose --profile sessions up -d --no-build --remove-orphans
```

Omit `--profile sessions` when you use only port 9222. Existing sessions end on
redeployment; create new ones through the same API. The new containers run as an
unprivileged user and need no persistent volumes. Old `proxy-state`, `proxy-trust`
and `session-state` volumes are unused; leave them in place until you are happy
with the upgrade. Remove them deliberately using your own project's volume names.
No volume deletion is part of the migration commands.

## CLI

Install the release bundle, which contains `camopanda`, `camopanda-gateway` and
its patched `lightpanda` executable together. An explicit `--lightpanda PATH` or
`LIGHTPANDA_BIN` still selects a browser, but Camopanda now checks for the required
patch capability and refuses stock or incompatible builds.

Commands, output and ordinary exit codes still pass through. `camopanda --version`
now reports Camopanda's version; use `lightpanda version` for the browser version.
`--state-dir` is accepted with a deprecation message and creates no files. Old
certificate directories are unused and are not automatically deleted.

The gateway no longer uses `CAMOPANDA_BIN` or `SESSION_STATE_DIR`; select its
browser with `LIGHTPANDA_BIN`. Clients cannot override the session user agent
through CDP. `--http-proxy` and `--ca-cert` are now ordinary upstream options,
only needed when you explicitly use your own proxy or private CA.

## Rollback

Keep the previous release's Compose file, environment and images, or your previous
CLI bundle. Stop the new stack before restarting the saved stack to avoid port
conflicts. Existing certificate volumes remain available. Restore the prior CLI
and browser together. The release history and Git tags preserve previous source;
a source checkout that uses upstream `nightly` needs the previously tested image
rather than a newly pulled nightly to reproduce the old deployment exactly.
