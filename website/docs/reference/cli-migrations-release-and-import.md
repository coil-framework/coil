---
title: CLI Migrations, Release, And Import
---

This page covers the commands that shape rollout and change-management.

## Migration Commands

The platform exposes:

```text
platform migrate plan
platform migrate apply
```

A real example from the current Shoppr config:

```bash
cargo run -p davenda-cli -- migrate plan --config apps/shoppr/platform.dev.toml
```

The output is a composed plan with owners such as:

- `module:commerce`
- `module:cms`
- `module:events`
- `auth:shoppr-auth`

That is what makes Davenda migration planning useful: you see one composed contract, not a pile of
unexplained SQL files.

For customer-app lifecycle, use the customer binary:

```bash
cd apps/shoppr
cargo run -p shoppr -- migrate apply --dry-run
```

That path validates the customer app bootstrap first. If required secrets such as
`OBJECT_STORE_URL` are missing, the customer binary fails early instead of pretending the runtime is
ready.

## Release Commands

The platform exposes:

```text
platform release doctor
platform release plan
```

Use them like this:

```bash
cargo run -p davenda-cli -- release doctor --config apps/shoppr/platform.dev.toml
cargo run -p davenda-cli -- release plan --config apps/shoppr/platform.dev.toml
```

Use `release doctor` when you need a fast readiness diagnostic. Use `release plan` when you need
the fuller composed release shape.

## Import Commands

The platform exposes:

```text
platform import run <manifest-path>
platform import cutover <manifest-path>
```

Cutover supports several explicit modes:

- `--apply`
- `--switch`
- `--observe`
- `--rollback`

Example:

```bash
cargo run -p davenda-cli -- import cutover imports/wordpress-events.toml \
  --switch \
  --base-url https://shop.example.com \
  --dns-zone-id zone_123 \
  --dns-target davenda-origin.example.net \
  --yes
```

## Read Next

- [Migration Files And Ownership](./migration-files-and-ownership.md)
- [CLI Cache, Jobs, TLS, Storage, And Assets](./cli-cache-jobs-storage-and-assets.md)
