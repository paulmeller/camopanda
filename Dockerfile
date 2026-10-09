FROM rust:1.93-trixie AS browser-build
RUN apt-get update && apt-get install -y --no-install-recommends python3 git curl make clang xz-utils ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /project
COPY browser browser
COPY scripts/build-browser.py scripts/build-browser.py
ARG BROWSER_RUN_TESTS=0
RUN if [ "$BROWSER_RUN_TESTS" = 1 ]; then python3 scripts/build-browser.py --work /work --output /out --test; else python3 scripts/build-browser.py --work /work --output /out; fi

FROM rust:1.93-bookworm AS cli-build
WORKDIR /src
COPY cli/Cargo.toml cli/Cargo.lock ./
COPY cli/src ./src
RUN cargo build --release --locked --bins

FROM debian:trixie-slim AS browser
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates libstdc++6 && rm -rf /var/lib/apt/lists/* && useradd --uid 10001 --create-home camopanda
COPY --from=browser-build /out/lightpanda /usr/local/bin/lightpanda
COPY --from=browser-build /out/licenses /usr/local/share/licenses
COPY --from=browser-build /out/lightpanda-source.tar.gz /usr/local/share/camopanda/lightpanda-source.tar.gz
COPY --from=browser-build /out/browser-pins.json /usr/local/share/camopanda/browser-pins.json
COPY --from=cli-build /src/target/release/camopanda /usr/local/bin/camopanda
COPY --from=cli-build /src/target/release/camopanda-gateway /usr/local/bin/camopanda-gateway
COPY LICENSE /usr/local/share/licenses/camopanda/LICENSE
LABEL org.opencontainers.image.licenses="MIT AND AGPL-3.0-only"
LABEL org.opencontainers.image.source="https://github.com/paulmeller/camopanda"
ENV LIGHTPANDA_DISABLE_TELEMETRY=true
USER camopanda
EXPOSE 9222
ENTRYPOINT ["camopanda"]
CMD ["serve", "--host", "0.0.0.0", "--port", "9222"]

FROM browser AS gateway
ENV SESSION_BIND=0.0.0.0:9223
EXPOSE 9223
ENTRYPOINT ["camopanda-gateway"]
CMD []
