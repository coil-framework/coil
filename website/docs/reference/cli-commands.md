---
title: CLI Commands
---

Coil has three CLI surfaces:

- the project lifecycle CLI, `cargo-coil`
- the root platform CLI, `coil-cli`
- customer-owned workspace binaries such as `shoppr`, `gitly`, or `my-store`

Use this page when you want to answer:

- which commands belong to `cargo coil`
- which commands the platform CLI already supports
- when to use the customer binary instead
- which commands are safe to expose in docs and automation
- why there are multiple Coil CLI surfaces at all

## One Platform, Three CLI Surfaces

A new Coil developer should treat the split like this:

- `cargo coil`
  - create and evolve the customer workspace
  - write and reconcile `.coil/project.toml`
  - add modules, sites, and locales
- `coil`
  - generic operator and infrastructure commands that work across customer apps
- `shoppr`, `gitly`, or `my-store`
  - customer-app-shaped commands that know the current app’s templates, extensions, linked backend, and bootstrap

The split exists because Coil has three distinct concerns:

- project generation
- platform operations
- app-local lifecycle

If you are starting a new product, begin with `cargo coil`. If you are inside a customer app
workspace, start with the customer binary first. Use `coil` when you need deeper operator
workflows such as import, cutover, cache, TLS, jobs, or auth inspection.

## `cargo coil`

The `cargo-coil` crate builds the Cargo subcommand:

```text
cargo coil new
cargo coil init
cargo coil apply
cargo coil doctor
cargo coil module add|remove
cargo coil site add
cargo coil locale add
```

These commands own the customer workspace shape.

Detailed command pages:

- [Cargo Coil Overview](./cargo-coil-overview.md)
- [cargo coil new](./cargo-coil-new.md)
- [cargo coil init](./cargo-coil-init.md)
- [cargo coil apply](./cargo-coil-apply.md)
- [cargo coil doctor](./cargo-coil-doctor.md)
- [cargo coil module add and remove](./cargo-coil-module.md)
- [cargo coil site add](./cargo-coil-site.md)
- [cargo coil locale add](./cargo-coil-locale.md)

## The Relationship Between The CLI Surfaces

`cargo coil` is project-shaped:

- workspace generation
- descriptor-backed regeneration
- structural edits such as sites and locales

The platform CLI is operator-shaped:

- auth
- modules
- cache
- jobs
- TLS
- storage
- imports
- release planning

The customer binary is runtime-shaped for one actual app:

- validate the customer workspace
- describe the customer composition
- run app-specific asset and migration flows
- expose app-specific diagnostics such as linked-backend or extension checksums

None of these replaces the others:

- `cargo coil` does not replace `coil`
- `coil` does not replace the customer binary
- the customer binary does not replace `cargo coil`

## Root Platform CLI

The `coil-cli` crate currently builds the `coil` binary. Real help output starts like this:

```text
coil dev server [--config <path>]
coil config validate [--config <path>] [--json]
coil auth check [--config <path>] --subject <subject> --capability <capability> --resource <namespace:id> [--json]
coil module list [--config <path>] [--json]
coil migrate plan [--config <path>] [--json]
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
# project-shaped
cargo run -p cargo-coil -- new my-store

# app-shaped
cd apps/shoppr
cargo run -p shoppr -- validate

# platform-shaped
cargo run -p coil-cli -- jobs status --config apps/shoppr/platform.dev.toml
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

The command registry in `crates/coil-cli/src/command.rs` already marks install, enable, and
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

This is one of the clearest signs that Coil treats jobs as a real operator surface, not just a
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

The import cutover flags are modeled in `crates/coil-cli/src/cli/import.rs`, including:

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
