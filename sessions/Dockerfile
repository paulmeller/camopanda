FROM rust:1.93-bookworm AS build
WORKDIR /src
COPY proxy/Cargo.toml proxy/Cargo.lock ./
COPY proxy/src ./src
RUN cargo build --release --locked --bin camopanda --bin camopanda-gateway

FROM lightpanda/browser:nightly
COPY --from=build /src/target/release/camopanda /usr/local/bin/camopanda
COPY --from=build /src/target/release/camopanda-gateway /usr/local/bin/camopanda-gateway
COPY LICENSE /usr/local/share/licenses/camopanda/LICENSE
USER root
RUN mkdir -p /state && chown lightpanda:lightpanda /state && chmod 0700 /state
USER lightpanda
ENV SESSION_BIND=0.0.0.0:9223 SESSION_STATE_DIR=/state
ENTRYPOINT ["/usr/local/bin/camopanda-gateway"]
