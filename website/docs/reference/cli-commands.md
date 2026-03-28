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

## Root Platform CLI

The canonical command registry lives in `crates/davenda-cli/src/command.rs`.

The parser lives in:

- `crates/davenda-cli/src/cli/args.rs`
- `crates/davenda-cli/src/cli/import.rs`

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

## Cache Commands

Current cache commands:

- `cache warm`
- `cache inspect`
- `cache invalidate`

Concrete parser behaviour in `crates/davenda-cli/src/cli/args.rs` already enforces:

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

The command model in `crates/davenda-cli/src/command.rs` exposes three important behaviours:

- `supports_json`
- `supports_dry_run`
- `requires_confirmation`

That means the CLI is intentionally machine-facing as well as human-facing.

## Customer Workspace Binaries

The demo apps also own their lifecycle through customer binaries.

Concrete entrypoints:

- `apps/shoppr/crates/shoppr-bin/src/main.rs`
- `apps/gitly/crates/gitly-bin/src/main.rs`

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

- [Environment Variables](./environment-variables.md)
- [Migration Files And Ownership](./migration-files-and-ownership.md)
- [Shoppr Jobs, Webhooks, And Background Work](../use-cases/shoppr/jobs-webhooks-and-background-work.md)
- [Gitly Build And Deploy](../use-cases/gitly/build-and-deploy.md)
