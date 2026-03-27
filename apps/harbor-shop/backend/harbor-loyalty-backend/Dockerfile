FROM rust:1.88-bookworm AS builder

WORKDIR /workspace
ENV CARGO_NET_GIT_FETCH_WITH_CLI=true
ENV CARGO_HTTP_TIMEOUT=600
ENV CARGO_NET_RETRY=10
ENV CARGO_TARGET_DIR=/tmp/cargo-target

COPY . .

RUN sh apps/harbor-shop/scripts/prepare-local-dev.sh

WORKDIR /workspace/apps/harbor-shop

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/tmp/cargo-target \
    cargo build -p harbor-loyalty-backend --release \
    && cp /tmp/cargo-target/release/harbor-loyalty-backend /workspace/harbor-loyalty-backend-bin

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

ENV HARBOR_BACKEND_BIND=0.0.0.0:8081
WORKDIR /app

COPY --from=builder /workspace/harbor-loyalty-backend-bin /usr/local/bin/harbor-loyalty-backend

ENTRYPOINT ["harbor-loyalty-backend"]
