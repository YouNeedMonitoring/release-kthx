FROM rust:1.93.1-bookworm AS builder
WORKDIR /build

COPY Cargo.toml Cargo.toml
COPY Cargo.lock Cargo.lock
COPY crates crates
COPY src src
RUN cargo build --release --locked

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates git gh \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /build/target/release/release-kthx /usr/local/bin/release-kthx
COPY entrypoint.sh /entrypoint.sh

ENTRYPOINT ["/entrypoint.sh"]
