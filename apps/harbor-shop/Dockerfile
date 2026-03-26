FROM rust:1.85-bookworm AS builder

WORKDIR /workspace

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        git \
        libssl-dev \
        pkg-config \
    && rm -rf /var/lib/apt/lists/*

RUN git config --global url."https://github.com/".insteadOf ssh://git@github.com/

COPY .cargo ./.cargo
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY apps ./apps

RUN cargo build -p davenda-cli --release

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        curl \
        netcat-openbsd \
        postgresql-client \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app/apps/harbor-shop

COPY --from=builder /workspace/target/release/platform /usr/local/bin/platform
COPY --from=builder /workspace/apps /app/apps
COPY apps/harbor-shop/docker/entrypoint.sh /usr/local/bin/harbor-shop-entrypoint

RUN chmod +x /usr/local/bin/harbor-shop-entrypoint

ENTRYPOINT ["harbor-shop-entrypoint"]
