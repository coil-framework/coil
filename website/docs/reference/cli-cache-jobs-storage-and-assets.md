---
title: CLI Cache, Jobs, TLS, Storage, And Assets
---

This page covers the operational command families that are not tied to one customer app’s custom
business logic.

## Cache Commands

```text
coil cache warm
coil cache inspect
coil cache invalidate
```

Examples:

```bash
cargo run -p coil-cli -- cache warm \
  --config apps/shoppr/platform.dev.toml \
  --scope public \
  --route /en-GB/shop
```

```bash
cargo run -p coil-cli -- cache inspect \
  --config apps/shoppr/platform.dev.toml \
  --route /en-GB/shop
```

```bash
cargo run -p coil-cli -- cache invalidate \
  --config apps/shoppr/platform.dev.toml \
  --tag route:events.list \
  --tag locale:en-GB \
  --yes
```

Use them for different moments:

- `cache warm`
  - prime a route before traffic arrives
- `cache inspect`
  - inspect the current cache state for one route
- `cache invalidate`
  - force recomputation after publishing or a product/content change

## Jobs Commands

```text
coil jobs status
coil jobs run
coil jobs ready
coil jobs dead-letters
coil jobs in-flight
coil jobs retry
coil jobs promote
```

Examples:

```bash
cargo run -p coil-cli -- jobs status --config apps/shoppr/platform.dev.toml
cargo run -p coil-cli -- jobs run --config apps/shoppr/platform.dev.toml --worker-id worker-a --limit 25
```

Use `dead-letters`, `retry`, and `promote` when you are handling recovery, not during routine local development.

Typical operator workflow:

1. `jobs status`
2. `jobs in-flight`
3. `jobs dead-letters`
4. `jobs retry` or `jobs promote`

## TLS, Storage, And Assets

```text
coil tls status
coil tls validate-challenge
coil tls renew
coil storage inspect
coil storage verify
coil assets publish
```

Examples:

```bash
cargo run -p coil-cli -- tls status --config apps/shoppr/platform.dev.toml
cargo run -p coil-cli -- storage inspect --config apps/shoppr/platform.dev.toml
cargo run -p coil-cli -- assets publish --config apps/shoppr/platform.dev.toml --dry-run
```

Read those commands like this:

- `tls status`
  - inspect certificate and challenge state
- `tls validate-challenge`
  - prove the edge challenge path is satisfiable
- `tls renew`
  - inspect or force renewal work
- `storage inspect`
  - inspect the effective storage topology and policy
- `storage verify`
  - prove the storage backend is reachable and policy-compliant
- `assets publish`
  - publish the hashed asset set the runtime will serve

These are platform-level operator commands. If you want the customer-app-shaped publication flow,
use the customer binary instead:

```bash
cd apps/shoppr
cargo run -p shoppr -- assets publish
```

## Read Next

- [CLI Commands](./cli-commands.md)
- [Customer Workspace Binaries](./customer-workspace-binaries.md)
