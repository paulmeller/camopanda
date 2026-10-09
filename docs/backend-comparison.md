# Local backend comparison

Measured on macOS arm64 using the same pinned, patched Lightpanda executable for
both CLI backends. The previous CLI ran it in normal mode behind Hudsucker; the
new CLI enabled its header profile mode and connected directly. Both Camopanda
wrappers were debug builds; the browser was a release build.

Three fresh processes per backend, alternating order. Each run warmed three
navigation/fetch requests, then measured 20 local HTTP navigation and fetch
samples. RSS sums the wrapper and browser process tree after startup and after
requests. It is not peak memory, a TLS measurement or a production capacity test.

| Median | Previous proxy backend | Direct backend |
|---|---:|---:|
| Idle tree RSS | 39.28 MiB | 35.11 MiB |
| Loaded tree RSS | 45.75 MiB | 40.03 MiB |
| Navigation completion | 1.558 ms | 1.364 ms |
| JavaScript fetch | 0.295 ms | 0.110 ms |

Loaded RSS was 12.5% lower in this fixture. Navigation includes CDP round trips;
fetch timing is measured inside JavaScript. These small local timings should not
be extrapolated to remote websites. Both CLI runs had two processes. The new
session gateway launches the browser directly and also removes its previous
per-session wrapper, but this comparison did not measure gateway capacity.

To reproduce, build the CLI from Git revision `39e2076` separately, build the new
CLI, and use the same patched browser for both:

```sh
LEGACY_CAMOPANDA=/path/to/old/proxy/target/debug/camopanda \
LIGHTPANDA_BIN=/path/to/patched/lightpanda \
node scripts/benchmark-backends.mjs
```

The script prints individual samples and medians, checks outgoing user agents,
and cleans up its test browser processes and temporary certificate directories.
