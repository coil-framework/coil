---
title: CLI Commands
---

Davenda ships a single CLI surface for development, validation, operations, and release work.

This page documents the command groups visible in `crates/davenda-cli/src/cli/args.rs` and shows
how they map to common workflows.

## How To Read This Page

Use this page for command discovery and examples. Then follow the linked operations pages for
deeper narrative.

## Global Behaviour

Important global flags:

- `--config <path>`
  - choose the platform config file explicitly
- `--json`
  - render machine-readable output
- `--dry-run`
  - preview mutating commands where supported
- `--yes`
  - confirm mutating commands non-interactively

If `--config` is omitted, the CLI will try:

1. `DAVENDA_CONFIG`
2. a default config file discoverable from the current app root

## Development

### `dev server`

Starts the local runtime against the chosen config.

Common pattern:

```bash
cargo run -p shoppr-bin -- dev server --config platform.dev.toml
```

## Configuration And Validation

### `config validate`

Validates the effective platform configuration and app alignment.

Example:

```bash
cargo run -p shoppr-bin -- config validate --config platform.dev.toml
```

## Auth

Current auth commands:

- `auth explain`
- `auth check`
- `auth bindings inspect`
- `auth test-model`
- `auth list`
- `auth lookup`
- `auth package validate`
- `auth package inspect`

Example:

```bash
cargo run -p shoppr-bin -- auth explain \
  --config platform.toml \
  --subject user:alice \
  --capability cms.page.publish \
  --resource page:membership-guide
```

## Modules

Current module commands:

- `module list`
- `module inspect`
- `module install`
- `module enable`
- `module disable`

Use them to understand or adjust which official modules participate in the customer app.

## Migrations And Release Checks

Current commands:

- `migrate plan`
- `migrate apply`
- `release doctor`
- `release plan`

Use these before cutover or production releases.

## Cache

Current cache commands:

- `cache warm`
- `cache inspect`
- `cache invalidate`

Examples:

```bash
cargo run -p shoppr-bin -- cache warm --config platform.toml --scope site --route /
cargo run -p shoppr-bin -- cache invalidate --config platform.toml --tag product:shoppr-rain-shell
```

## Jobs

Current jobs commands:

- `jobs status`
- `jobs run`
- `jobs ready`
- `jobs dead-letters`
- `jobs in-flight`
- `jobs retry`
- `jobs promote`

These map directly to the operational workflows documented in
[Jobs And Schedulers](../operations/jobs-and-schedulers.md).

## TLS And Storage

Current TLS and storage commands:

- `tls status`
- `tls validate-challenge`
- `tls renew`
- `storage inspect`
- `storage verify`
- `assets publish`

Use them to verify asset publication and TLS readiness before cutover.

## Import And Cutover

Current import and cutover commands:

- `import run`
- `import cutover`

Cutover supports plan, apply, observe, switch, and rollback style flows through flags such as:

- `--apply`
- `--switch`
- `--observe`
- `--rollback`

## Shoppr And Gitly Examples

The canonical customer binaries are:

- `apps/shoppr/crates/shoppr-bin/src/main.rs`
- `apps/gitly/crates/gitly-bin/src/main.rs`

Use those together with:

- [Build And Deploy](../operations/build-and-deploy.md)
- [Cache, TLS, Cutover, And Rollback](../operations/cache-tls-cutover-and-rollback.md)
- [Jobs And Schedulers](../operations/jobs-and-schedulers.md)

## Common Mistakes

- Running mutating commands without `--dry-run` first in production.
- Omitting `--config` and assuming the CLI will find the right app root.
- Treating `release doctor` as optional instead of part of the release contract.
- Forgetting that some live queue and cutover operations require backing infrastructure such as
  `DATABASE_URL` or cache URLs.

## Read Next

- [Environment Variables](./environment-variables.md)
- [Build And Deploy](../operations/build-and-deploy.md)
- [Jobs And Schedulers](../operations/jobs-and-schedulers.md)
