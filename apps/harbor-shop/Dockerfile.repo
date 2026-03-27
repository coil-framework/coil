FROM rust:1.88-bookworm AS builder

WORKDIR /workspace
ENV CARGO_NET_GIT_FETCH_WITH_CLI=true
ENV CARGO_HTTP_TIMEOUT=600
ENV CARGO_NET_RETRY=10
ENV CARGO_TARGET_DIR=/tmp/cargo-target

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        git \
        libssl-dev \
        pkg-config \
    && rm -rf /var/lib/apt/lists/*

RUN git config --global --add url."https://github.com/".insteadOf "ssh://git@github.com/" \
    && git config --global --add url."https://github.com/".insteadOf "git@github.com:" \
    && git config --global --add url."https://github.com/worka-ai/".insteadOf "ssh://git@github.com/worka-ai/"

COPY . .

RUN sh apps/harbor-shop/scripts/prepare-local-dev.sh

WORKDIR /workspace/apps/harbor-shop

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/tmp/cargo-target \
    cargo build --locked --manifest-path /workspace/Cargo.toml -p davenda-cli --release \
    && cargo build --locked -p harbor-shop --release \
    && cp /tmp/cargo-target/release/platform /workspace/platform \
    && cp /tmp/cargo-target/release/harbor-shop /workspace/harbor-shop

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        curl \
        netcat-openbsd \
        postgresql-client \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app/apps/harbor-shop

COPY --from=builder /workspace/platform /usr/local/bin/platform
COPY --from=builder /workspace/harbor-shop /usr/local/bin/harbor-shop
COPY --from=builder /workspace/apps /app/apps
COPY apps/harbor-shop/docker/entrypoint.sh /usr/local/bin/harbor-shop-entrypoint

RUN chmod +x /usr/local/bin/harbor-shop-entrypoint

ENTRYPOINT ["harbor-shop-entrypoint"]
