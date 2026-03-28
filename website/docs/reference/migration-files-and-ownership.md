---
title: Migration Files And Ownership
---

Davenda treats migrations as part of the customer app contract, not as hidden framework magic.

That means customer teams can add their own schema changes, but they should do so deliberately and
with clear ownership boundaries.

## What This Page Covers

Use this page when you need to know:

- where migration files live
- how customer-owned migrations fit alongside framework-owned ones
- whether you can add your own tables
- how to version migrations safely

## The Basic Rule

Yes, customer apps can add their own tables and schema changes.

The important constraint is ownership:

- framework-owned migrations should continue to own framework tables
- auth packages own auth schema migrations
- customer-owned crates or app packages should clearly own customer tables

Do not casually edit framework migrations to add customer-specific storage.

## Canonical Example

Shoppr already shows one migration-owned surface in the auth package:

- `apps/shoppr/auth/shoppr-auth/migrations/001_bootstrap.sql`

That demonstrates the basic Davenda pattern:

- migrations are checked in as ordinary files
- numbering is explicit
- ownership lives with the package that owns the schema

## Where Customer Migrations Should Live

Use a location that makes ownership obvious.

Good patterns:

- a dedicated customer backend or app crate migration directory
- an auth package migration directory when the migration belongs to auth
- a module-owned migration directory when the schema belongs to a module

Avoid:

- sprinkling unrelated SQL files around the repo root
- hiding customer schema behind framework-owned migration names

## Numbering And Versioning

Use explicit ordered migration names such as:

```text
001_create_crm_contacts.sql
002_add_membership_tier.sql
003_backfill_waitlist_source.sql
```

The rule is not the exact prefix width. The rule is that ordering must be obvious and deterministic.

## What A Customer Table Might Look Like

A customer backend might need a table for:

- CRM sync receipts
- loyalty programme state
- external integration mapping ids
- warehouse-specific inventory import markers

Those are legitimate customer-owned data shapes and should not be forced into generic CMS or order
tables if they are real backend concerns.

## Operational Workflow

In practice:

1. add the migration file
2. run `migrate plan`
3. review the plan
4. apply in development with `migrate apply`
5. include migration rollout in the release and cutover plan

See [Build And Deploy](../operations/build-and-deploy.md) and
[Database Migrations](../operations/database-migrations.md) for the operational flow.

## How This Relates To Linked Rust

Linked Rust backends can absolutely depend on customer-owned tables.

The right model is:

- linked Rust owns the business logic
- customer migrations own the extra schema it needs

That is normal. The mistake would be pretending the backend needs no durable storage and then
smuggling state through unrelated framework tables.

## Common Mistakes

- Editing framework-owned migrations to add customer columns.
- Using opaque migration names that hide intent.
- Treating customer tables as forbidden when the backend genuinely needs them.
- Forgetting to document which crate or package owns a migration set.

## Read Next

- [Database Migrations](../operations/database-migrations.md)
- [Linked Rust Hook APIs](./linked-rust-hook-apis.md)
- [Build And Deploy](../operations/build-and-deploy.md)
