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
    cd /workspace/apps/gitly \
    && cargo build --locked -p gitly --release \
    && cp /tmp/cargo-target/release/gitly /workspace/gitly

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        curl \
        netcat-openbsd \
        postgresql-client \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /workspace/gitly /usr/local/bin/gitly
COPY --from=builder /workspace/apps/gitly /app
COPY apps/gitly/docker/entrypoint.sh /usr/local/bin/gitly-entrypoint

RUN chmod +x /usr/local/bin/gitly-entrypoint

ENTRYPOINT ["gitly-entrypoint"]
