---
title: Migration Files And Ownership
---

Davenda migrations are intentionally split by owner. That is what keeps module upgrades, auth
validation, and customer-app changes from collapsing into one undocumented SQL pile.

Use this page when you want to answer:

- who owns which migration step
- where those steps are declared
- why customer-app migrations can be manual even when module migrations are executable

## The Ownership Model

The low-level migration owner enum lives in `crates/davenda-data/src/migration.rs`.

Current owners:

- `Core`
- `Module(String)`
- `CustomerApp(String)`
- `AuthPackage(String)`

The higher-level app migration summary lives in `crates/davenda-app/src/migration.rs`.

The customer-app-facing summary categories are:

- `Module`
- `AuthPackage`
- `CustomerApp`

## What Each Owner Means

### Module migrations

These are executable migration steps shipped by official or customer-linked modules.

The runtime composes them through module manifests and migration plans before `migrate apply`.

### Auth package migrations

Auth packages contribute validation and compatibility checks to the migration/release story.

In the plan summary they appear as:

- `auth:<package>`

That means "validate this auth package boundary before release", not "run arbitrary hidden SQL".

### Customer app migrations

These are the steps the customer app itself owns and must explain explicitly.

That is why the demo customer binaries report `manual_customer_migration_entries`.

## Where Migration Plans Are Built

The important files are:

- `crates/davenda-data/src/migration.rs`
  - executable migration steps and compiled batches
- `crates/davenda-app/src/migration.rs`
  - composed migration summaries for customer apps
- `apps/shoppr/crates/shoppr-app/src/lib.rs`
  - Shoppr workspace validation and apply flow
- `apps/gitly/crates/gitly-app/src/lib.rs`
  - Gitly workspace validation and apply flow

## Shoppr Example

Shoppr’s workspace binary is the clearest example because it reports both executable and manual
customer-owned migration work.

Read:

- `apps/shoppr/crates/shoppr-app/src/lib.rs`
- `apps/shoppr/crates/shoppr-bin/src/main.rs`

Important behaviors:

- `validate(...)` reports `manual_customer_migration_entries`
- `migrate_apply(...)` builds the runtime bootstrap and applies executable steps
- customer-owned manual entries are derived from the composed manifest and still surfaced to the
  operator explicitly

That is the right migration story for a real product: executable where possible, explicit where
human ownership is still required.

## Gitly Example

Gitly follows the same pattern:

- `apps/gitly/crates/gitly-app/src/lib.rs`
- `apps/gitly/crates/gitly-bin/src/main.rs`

This matters because Gitly proves migration ownership is not a commerce-only concept.

## Ordering Rules

The low-level ordering in `crates/davenda-data/src/migration.rs` is:

1. core
2. module
3. auth package
4. customer app

The app-facing summary in `crates/davenda-app/src/migration.rs` keeps the relevant subset:

1. module
2. auth package
3. customer app

That ordering is deliberate. The more reusable or infrastructural owners run earlier; the most
product-specific owner stays last.

## What A Customer App Should Commit

A customer app should commit:

- module enablement in `app.toml`
- customer migration declarations in the app manifest/composition layer
- a customer binary that exposes validate and migrate commands

Concrete demo binaries:

- `apps/shoppr/crates/shoppr-bin/src/main.rs`
- `apps/gitly/crates/gitly-bin/src/main.rs`

## Common Mistakes

- Do not hide customer-owned migration work inside ad hoc README notes only.
- Do not treat auth-package validation as optional if the migration plan surfaces it.
- Do not assume every migration step is executable SQL.
  - some are explicit ownership checkpoints
- Do not skip the customer binary lifecycle layer if you want a third-party developer story.

## Read Next

- [CLI Commands](./cli-commands.md)
- [Composition And davenda-all](./composition.md)
- [Shoppr Checkout And Operations](../use-cases/shoppr/checkout-and-operations.md)
