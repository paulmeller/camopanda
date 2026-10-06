FROM rust:1.93-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY tests ./tests
RUN cargo build --release --locked --bin hudsucker-proxy

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates openssl \
    && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/target/release/hudsucker-proxy /usr/local/bin/hudsucker-proxy
COPY entrypoint.sh /usr/local/bin/hudsucker-entrypoint
RUN chmod 0755 /usr/local/bin/hudsucker-entrypoint
ENTRYPOINT ["/usr/local/bin/hudsucker-entrypoint"]
