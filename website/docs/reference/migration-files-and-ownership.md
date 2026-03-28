---
title: Migration Files And Ownership
---

Davenda migrations are intentionally split by owner. That is what keeps module upgrades, auth
validation, and customer-app changes from collapsing into one undocumented SQL pile.

Use this page when you want to answer:

- who owns which migration step
- where those steps are declared
- why customer-app migrations can be manual even when module migrations are executable
- how to run the real commands and interpret the output

## The Practical Answer

If your linked backend or customer app needs extra tables, projections, or backfills, the answer
today is:

- yes, customer-owned migration work is first-class in the composed plan
- no, Davenda does not yet auto-run arbitrary customer SQL files for you

That is why migration ownership is explicit in the docs. The platform records and surfaces the
contract; the customer app still owns the concrete rollout step for purely customer-managed schema
work.

## The Ownership Model

Current owners are:

- `Core`
- `Module(String)`
- `CustomerApp(String)`
- `AuthPackage(String)`

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

## What The Platform CLI Shows You

The platform CLI gives you the composed migration plan across modules and auth.

Real command:

```bash
cargo run -p davenda-cli -- migrate plan --config apps/shoppr/platform.dev.toml
```

Real output includes rows like:

```text
owner: module:commerce
step: 001_catalog_products
order: 10
online_safe: true
description: create catalog products and variants tables
```

and:

```text
owner: auth:shoppr-auth
step: version-check
order: 0
online_safe: true
description: validate auth package `shoppr-auth` schema, model, and capability bindings before release
```

That tells you two important things:

- executable module work is visible as planned steps
- auth-package validation is also part of the migration contract even when it is not raw SQL

## Shoppr Example

Shoppr’s workspace binary is the clearest example because it reports both executable and manual
customer-owned migration work.

Real command:

```bash
cd apps/shoppr
cargo run -p shoppr -- validate
```

Real output today:

```text
Shoppr validation passed
app id: shoppr
route surfaces: 49
jobs: 16
migration contracts: 20
manual customer migrations: none
```

That is the honest current state of the demo:

- Shoppr has real composed migration contracts
- Shoppr currently has no manual customer migration entries

## How To Declare A Customer Migration

The manifest shape is:

```toml
[[customer_migrations]]
id = "customer.content"
order = 90
description = "Creates customer app landing-page projections"
```

Each field means:

- `id`
  - the stable operator-facing name for the migration entry
- `order`
  - where it appears relative to module and auth-owned steps
- `description`
  - the human explanation that shows up in planning and rollout output

Use a customer migration entry for:

- customer-owned projection tables
- backfills
- external-system cutover checkpoints
- manual schema work that belongs to the customer app rather than an official module

Real customer-binary behaviour:

- `validate(...)` reports `manual_customer_migration_entries`
- `migrate_apply(...)` builds the runtime bootstrap and applies executable steps
- customer-owned manual entries are derived from the composed manifest and still surfaced to the
  operator explicitly

That is the right migration story for a real product: executable where possible, explicit where
human ownership is still required.

## What The Customer Binary Adds

The customer binary owns the app-shaped migration workflow:

```bash
cd apps/shoppr
cargo run -p shoppr -- migrate apply --dry-run
```

If the app cannot build a valid runtime bootstrap, the command fails early. For example, the current
Shoppr dry-run path still requires runtime secrets such as `OBJECT_STORE_URL` to be present.

That is useful, not annoying. It stops the app from pretending it can apply migrations for a runtime
it cannot actually boot.

## Real Workflow For Customer-Owned Schema Changes

Use this sequence when your linked backend needs new schema or projection work:

1. add a `[[customer_migrations]]` entry to `app.toml`
2. describe the change clearly in `description`
3. run `platform migrate plan` to inspect the composed owner/order view
4. run `shoppr validate` or `gitly validate`
5. execute the actual customer-owned SQL or data step in your deployment workflow
6. re-run `release doctor` and `release plan`

That is the current Davenda workflow. The important thing is to be explicit about ownership rather
than pretending the platform already has a hidden arbitrary-SQL runner.

## Gitly Example

Gitly follows the same pattern:

- `gitly validate` also reports the composed migration-contract count
- `gitly migrate apply --dry-run` follows the same customer-binary ownership model

This matters because Gitly proves migration ownership is not a commerce-only concept.

## How Manual Customer Migrations Become Real

Customer migrations are declared through the customer app composition layer and surfaced into the
app-facing migration summary. That means:

- module migrations come from installed modules
- auth migration checks come from the selected auth package
- customer migration entries come from the customer app manifest/composition

The docs should emphasise the boundary, not force the reader to reverse-engineer it from core Rust.

## Ordering Rules

The low-level ordering is:

1. core
2. module
3. auth package
4. customer app

The app-facing summary keeps the relevant subset:

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

## Common Mistakes

- Do not hide customer-owned migration work inside ad hoc README notes only.
- Do not treat auth-package validation as optional if the migration plan surfaces it.
- Do not assume every migration step is executable SQL.
  - some are explicit ownership checkpoints
- Do not skip the customer binary lifecycle layer if you want a third-party developer story.

## Read Next

- [CLI Commands](./cli-commands.md)
- [CLI Migrations, Release, And Import](./cli-migrations-release-and-import.md)
- [Composition And davenda-all](./composition.md)
- [Shoppr Checkout And Operations](../use-cases/shoppr/checkout-and-operations.md)
