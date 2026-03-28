---
title: CLI Commands
---

Davenda has two CLI layers:

- the root platform CLI, `davenda-cli`
- customer-owned workspace binaries such as `shoppr` and `gitly`

Use this page when you want to answer:

- which commands the platform CLI already supports
- when to use the customer binary instead
- which commands are safe to expose in docs and automation
- why there is a customer binary at all

## One Platform, Two Binaries

A new Davenda developer should treat the split like this:

- `platform`
  - generic operator and infrastructure commands that work across customer apps
- `shoppr` or `gitly`
  - customer-app-shaped commands that know the current app’s templates, extensions, linked backend, and bootstrap

The split exists because Davenda has two audiences:

- platform operators and maintainers
- customer developers building one concrete product

If you are inside a customer app workspace, start with the customer binary first. Use `platform`
when you need deeper operator workflows such as import, cutover, cache, TLS, jobs, or auth
inspection.

## The Relationship Between The Two CLIs

There are two binaries because they solve different problems.

The platform CLI is generic:

- auth
- modules
- cache
- jobs
- TLS
- storage
- imports
- release planning

The customer binary is app-shaped:

- validate the customer workspace
- describe the customer composition
- run app-specific asset and migration flows
- expose app-specific diagnostics such as linked-backend or extension checksums

The customer binary does not replace the platform CLI, and the platform CLI does not replace the
customer binary.

## Root Platform CLI

The `davenda-cli` crate currently builds the `platform` binary. Real help output starts like this:

```text
platform dev server [--config <path>]
platform config validate [--config <path>] [--json]
platform auth check [--config <path>] --subject <subject> --capability <capability> --resource <namespace:id> [--json]
platform module list [--config <path>] [--json]
platform migrate plan [--config <path>] [--json]
```

The baseline command families are:

- `dev server`
- `config validate`
- `auth ...`
- `module ...`
- `migrate ...`
- `release ...`
- `cache ...`
- `jobs ...`
- `tls ...`
- `storage ...`
- `assets publish`
- `import run`
- `import cutover`

The fastest way to make the split real is to run one command from each layer:

```bash
# app-shaped
cd apps/shoppr
cargo run -p shoppr -- validate

# platform-shaped
cargo run -p davenda-cli -- jobs status --config apps/shoppr/platform.dev.toml
```

The first proves Shoppr can compose its own runtime. The second tells you what the platform job
system is doing for that app.

## Auth Commands

Current auth surfaces include:

- `auth check`
- `auth bindings inspect`
- `auth test-model`
- `auth list`
- `auth lookup`
- `auth explain`
- `auth package validate`
- `auth package inspect`

These are grounded in the live auth package and runtime state, not just static files.

Each auth subcommand now has its own detailed page:

- [CLI Auth And Module Commands](./cli-auth-and-modules.md)

## Module Commands

Current module operator commands:

- `module list`
- `module inspect`
- `module install`
- `module enable`
- `module disable`

The command registry in `crates/davenda-cli/src/command.rs` already marks install, enable, and
disable as:

- supporting dry-run
- requiring confirmation

That is the intended operator posture for composition-changing commands.

## Migration And Release Commands

Migration:

- `migrate plan`
- `migrate apply`

Release:

- `release doctor`
- `release plan`

Use these when you need the platform’s composed view across modules, auth, and customer-app
contracts.

Detailed command usage lives here:

- [CLI Migrations, Release, And Import](./cli-migrations-release-and-import.md)

## Cache Commands

Current cache commands:

- `cache warm`
- `cache inspect`
- `cache invalidate`

Concrete parser behaviour already enforces:

- `cache warm` requires at least one `--route`
- `cache inspect` requires exactly one `--route`
- `cache invalidate` requires at least one `--tag`

## Jobs Commands

Current jobs commands:

- `jobs status`
- `jobs run`
- `jobs ready`
- `jobs dead-letters`
- `jobs in-flight`
- `jobs retry`
- `jobs promote`

This is one of the clearest signs that Davenda treats jobs as a real operator surface, not just a
library feature.

Detailed usage lives here:

- [CLI Cache, Jobs, TLS, Storage, And Assets](./cli-cache-jobs-storage-and-assets.md)

## TLS, Storage, Assets, And Import

Current operator commands also include:

- `tls status`
- `tls validate-challenge`
- `tls renew`
- `storage inspect`
- `storage verify`
- `assets publish`
- `import run`
- `import cutover`

The import cutover flags are modeled in `crates/davenda-cli/src/cli/import.rs`, including:

- `--apply`
- `--switch`
- `--observe`
- `--rollback`
- DNS-specific target inputs

## Output, Dry Run, And Confirmation

The command model exposes three important behaviours:

- `supports_json`
- `supports_dry_run`
- `requires_confirmation`

That means the CLI is intentionally machine-facing as well as human-facing.

## Customer Workspace Binaries

The demo apps also own their lifecycle through customer binaries.

Actual Shoppr help output:

```text
Usage: shoppr [OPTIONS] <COMMAND>

Commands:
  describe
  validate
  assets
  migrate
  serve
  up
  linked-backend
```

Shoppr commands:

- `shoppr describe`
- `shoppr validate`
- `shoppr assets publish`
- `shoppr migrate apply`
- `shoppr serve`
- `shoppr up`
- `shoppr linked-backend ...`

Gitly commands:

- `gitly describe`
- `gitly validate`
- `gitly assets publish`
- `gitly migrate apply`
- `gitly serve`
- `gitly up`
- `gitly extension-checksums`
- `gitly linked-backend ...`

Use the customer binary when you want the actual app-shaped lifecycle a third-party developer
should run.

## The Practical Rule

Run commands in this order when you are new to a customer app:

1. `shoppr validate` or `gitly validate`
2. `shoppr describe` or `gitly describe`
3. `shoppr serve` or `gitly serve`
4. only then reach for `platform ...` if you need deeper operator work

That order matters because the customer binary proves the app can actually compose before you start
running lower-level platform commands against it.

## Practical Split

Use the root CLI when you need:

- platform-wide operator workflows
- import and cutover
- storage, cache, TLS, or auth diagnostics
- module operations

Use the customer binary when you need:

- app-shaped bootstrap
- app-local docs and tutorials
- app lifecycle that should stay customer-owned
- a third-party developer story that does not start from the monorepo root

## Read Next

- [Customer Workspace Binaries](./customer-workspace-binaries.md)
- [CLI Auth And Module Commands](./cli-auth-and-modules.md)
- [CLI Migrations, Release, And Import](./cli-migrations-release-and-import.md)
- [CLI Cache, Jobs, TLS, Storage, And Assets](./cli-cache-jobs-storage-and-assets.md)
- [Environment Variables](./environment-variables.md)
- [Migration Files And Ownership](./migration-files-and-ownership.md)
- [Shoppr Jobs, Webhooks, And Background Work](../use-cases/shoppr/jobs-webhooks-and-background-work.md)
- [Gitly Build And Deploy](../use-cases/gitly/build-and-deploy.md)
