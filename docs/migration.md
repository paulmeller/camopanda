# Upgrade from the proxy backend

Camopanda 0.2 replaces the rewriting proxy with a patched Lightpanda browser. Your configured
`UPSTREAM_USER_AGENT`, `CDP_BIND_IP`, and `CDP_PORT` settings keep the same meaning.
Browser request profiles remain distinct from browser/device emulation.

## Docker

Save your previous Compose file and its configuration. Stop the old stack before starting the new one:
the old services were named `proxy`, `lightpanda` and `sessions`; the new one is simply `browser`.
Use `--remove-orphans` with the updated Compose file after pulling/building it:

```sh
docker compose pull
docker compose up -d --no-build --remove-orphans
```

The new container runs as an unprivileged user and needs no persistent volumes. Old `proxy-state`,
`proxy-trust` and `session-state` volumes are unused; leave them in place until you are happy
with the upgrade. Remove them deliberately using your own project's volume names.
No volume deletion is part of the migration commands.

## CLI

Install the release bundle, which provides the patched `camopanda` / `lightpanda` executable.
Commands, options, and exit codes match Lightpanda 1:1.

Header profiles are enabled by setting `UPSTREAM_USER_AGENT` in the environment or by passing
`--user-agent <STRING>` with `--camopanda-header-profiles`.

The old `--state-dir` option is no longer needed; direct browser execution creates no certificates.
Old certificate directories are unused and are not automatically deleted.

## Rollback

Keep the previous release's Compose file, environment and images, or your previous CLI bundle.
Stop the new stack before restarting the saved stack to avoid port conflicts. Existing certificate
volumes remain available. Restore the prior CLI and browser together. The release history and
Git tags preserve previous source.
