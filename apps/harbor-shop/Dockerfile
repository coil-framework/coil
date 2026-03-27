FROM rust:1.88-bookworm AS builder

WORKDIR /workspace
ENV CARGO_NET_GIT_FETCH_WITH_CLI=true
ENV CARGO_HTTP_TIMEOUT=600
ENV CARGO_NET_RETRY=10
ENV CARGO_TARGET_DIR=/tmp/cargo-target

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        libssl-dev \
        pkg-config \
    && rm -rf /var/lib/apt/lists/*

COPY . .

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/tmp/cargo-target \
    cd /workspace/apps/harbor-shop \
    && cargo build --locked -p harbor-shop --release \
    && cp /tmp/cargo-target/release/harbor-shop /workspace/harbor-shop

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        curl \
        netcat-openbsd \
        postgresql-client \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /workspace/harbor-shop /usr/local/bin/harbor-shop
COPY --from=builder /workspace/apps/harbor-shop /app
COPY apps/harbor-shop/docker/entrypoint.sh /usr/local/bin/harbor-shop-entrypoint

RUN chmod +x /usr/local/bin/harbor-shop-entrypoint

ENTRYPOINT ["harbor-shop-entrypoint"]
