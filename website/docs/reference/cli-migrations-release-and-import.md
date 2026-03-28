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

If you are a customer-app developer, pair that with the customer binary:

```bash
cd apps/shoppr
cargo run -p shoppr -- migrate apply --dry-run
```

Use `platform migrate plan` to inspect the composed migration contract. Use `shoppr migrate apply
--dry-run` to prove the app bootstrap can actually execute it.

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

A practical sequence is:

1. `platform auth package validate`
2. `platform migrate plan`
3. `platform release doctor`
4. `platform release plan`

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

Use them literally:

- `--apply`
  - execute the prepared cutover step
- `--switch`
  - move traffic or source-of-truth state to the imported surface
- `--observe`
  - inspect readiness and cutover evidence without switching
- `--rollback`
  - revert to the previous side if the prepared cutover is not acceptable

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
