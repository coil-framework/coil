---
title: Prepare for Production
---

This chapter takes the checked-in Shoppr app from a local development stack to a production-shaped
runtime plan.

The goal is not to invent a fake deployment guide. The goal is to read the real files that already
separate local defaults from production behavior and to understand what each one is responsible for.

## Purpose

Use this chapter to answer four concrete questions:

- which values change between local and production runtime config?
- what does the customer binary actually do in a production-shaped flow?
- where do asset publication and startup sequencing happen?
- what remains customer-owned even when Coil provides the runtime?

The key files are:

- `apps/shoppr/platform.toml`
- `apps/shoppr/platform.dev.toml`
- `apps/shoppr/Dockerfile`
- `apps/shoppr/docker-compose.yml`
- `apps/shoppr/docker/entrypoint.sh`
- `apps/shoppr/crates/shoppr-bin/src/main.rs`

## Compare Development And Production Config First

The cleanest way to understand the production slice is to compare the same sections in the two
runtime config files.

Development uses local-friendly cookie settings and localhost-style infrastructure:

```toml
[server]
base_url = "http://localhost:8080"

[http.cookies]
secure = false

[observability]
metrics = true
tracing = true

[assets]
publish_manifest = true
cdn_base_url = "http://localhost:8080"
```

Production keeps the same general shape, but changes the values that matter for real traffic:

```toml
[server]
base_url = "https://shoppr.example.com"

[http.cookies]
secure = true

[tls]
mode = "acme"
challenge = "dns-01"
provider = "cloudflare-dns"

[storage]
local_root = "/var/lib/coil/shoppr"

[observability]
metrics = true
tracing = true

[assets]
publish_manifest = true
cdn_base_url = "https://cdn.example.com"
```

These blocks live in:

- `apps/shoppr/platform.dev.toml`
- `apps/shoppr/platform.toml`

What the important sections do:

- `[server]` selects the canonical public base URL
- `[http.cookies]` moves cookies from local dev behavior to secure production behavior
- `[tls]` defines the certificate strategy
- `[storage]` chooses a durable local state root on the host
- `[observability]` keeps the same probe and diagnostics shape in production
- `[assets]` moves asset publication onto a real CDN base URL

Exact next effect:

- you stop treating local config as if it were production-safe
- the runtime has a distinct production path for TLS, cookies, storage, and asset delivery

## Read The Customer Binary As The Production Command Surface

The real entry point for production operations is the Shoppr binary in
`apps/shoppr/crates/shoppr-bin/src/main.rs`.

The important part of the file is the command set:

```rust
enum Command {
    Describe,
    Validate,
    Assets {
        #[command(subcommand)]
        command: AssetsCommand,
    },
    Migrate {
        #[command(subcommand)]
        command: MigrateCommand,
    },
    Serve {
        #[arg(long)]
        bind: Option<String>,
    },
    Up {
        #[arg(long)]
        bind: Option<String>,
    },
}
```

Why this file exists:

- it is the executable operator boundary for the customer app
- it turns the app manifest and runtime config into runnable commands
- it keeps validation, asset publication, migrations, and serving on one binary path

What these commands enable:

- `describe` explains the assembled workspace
- `validate` checks the app and config shape before boot
- `assets publish` pushes theme assets through the asset pipeline
- `migrate apply` handles executable migrations
- `serve` and `up` start the app

Exact next effect:

- production prep becomes a visible command path, not a loose set of shell notes
- your deploy story can point at one customer-owned binary instead of undocumented framework steps

## Understand The Container Build And Startup Order

The production-shaped stack is split across the Dockerfile and the entrypoint.

The Dockerfile builds the app binary:

```dockerfile
FROM rust:1.90 AS builder
WORKDIR /workspace
COPY . .
RUN cargo build --release --manifest-path apps/shoppr/Cargo.toml -p shoppr
```

This lives in `apps/shoppr/Dockerfile`.

The entrypoint handles startup order:

```sh
wait_for "postgres" "$POSTGRES_HOST" "$POSTGRES_PORT"
wait_for "redis" "$REDIS_HOST" "$REDIS_PORT"
wait_for "minio" "$MINIO_HOST" "$MINIO_PORT"

/usr/local/bin/shoppr --config "$CONFIG_PATH" assets publish
exec /usr/local/bin/shoppr --config "$CONFIG_PATH" up
```

This lives in `apps/shoppr/docker/entrypoint.sh`.

What these files are doing:

- `Dockerfile` creates the production image with the customer binary
- `entrypoint.sh` waits for dependencies before boot
- `assets publish` happens before serving traffic
- `up` starts the runtime only after the startup prerequisites are met

Exact next effect:

- asset publication stops being a manual afterthought
- the app does not start racing Postgres, Redis, or object storage during local or production-shaped boot

## Use Compose To Understand The Runtime Topology

The local compose file is not production deployment, but it is the clearest checked-in topology map:

```yaml
services:
  shoppr:
    environment:
      COIL_CONFIG: platform.dev.toml
      DATABASE_URL: postgresql://shoppr:shoppr@db:5432/shoppr
      REDIS_URL: redis://cache:6379/0
      OBJECT_STORE_URL: s3://shoppr-assets
      COIL_COOKIE_SECRET: dev-cookie-secret
      COIL_CSRF_SECRET: dev-csrf-secret
```

This block lives in `apps/shoppr/docker-compose.yml`.

Why this file matters:

- it shows which infrastructure the app actually depends on
- it makes the secret and backing-service contract visible
- it gives you the handoff between config files and runtime environment variables

Exact next effect:

- you can see exactly which values must be externalized for a real deployment
- the customer app’s production requirements become inspectable instead of implicit

## What Coil Does And Does Not Do Automatically

Coil gives you:

- the runtime config format
- asset publication support
- migration execution support
- health and readiness behavior
- a customer binary that can describe, validate, publish, migrate, and boot the app

Coil does not automatically decide:

- your production hostname and CDN hostname
- your TLS provider choice
- your deployment platform
- your secret management system
- your storage durability and backup policy

Those remain customer-owned operational decisions. The checked-in Shoppr files show one coherent way
to express them, not the only possible deployment shape.

## Runnable Checkpoint

Validate the production-shaped config directly:

```bash
cargo run --manifest-path apps/shoppr/Cargo.toml -p shoppr -- --config platform.toml validate
```

Then inspect the production command path explicitly:

```bash
cargo run --manifest-path apps/shoppr/Cargo.toml -p shoppr -- --config platform.toml describe
```

If you want the full local stack with the same startup ordering the container uses:

```bash
docker compose -f apps/shoppr/docker-compose.yml up --build
```

What you should verify next:

- the app has a production config file separate from local defaults
- the binary exposes validation, asset publication, migrations, and startup as explicit commands
- the container startup sequence publishes assets before serving traffic
- the runtime dependencies and secrets are visible in the stack configuration

Exact next effect:

- after this chapter, your tutorial has a deployment-shaped runtime story instead of only a local
  dev story
- the reader can now see where `app.toml` stops, where runtime config begins, and where customer
  operational ownership still applies
