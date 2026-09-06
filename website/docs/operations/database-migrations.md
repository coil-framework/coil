---
title: Database Migrations
---

Coil treats migrations as an operator-visible lifecycle step, not as a side effect of startup.

## What Is This?

This page explains how to plan, apply, and reason about schema changes in Coil.

## Why Does This Matter?

Schema changes are one of the fastest ways to turn a healthy release into a broken rollout.

Coil intentionally separates:

- linked runtime composition
- executable migration application
- manual customer migration review

That makes rollback and incident reasoning much clearer.

## The Migration Model

A release can include:

- executable migrations supplied by core, modules, or customer app composition
- manual customer migration entries that still need deliberate handling

The checked-in customer binaries already surface both.

## Concrete Commands

Generic platform flow:

```bash
coil migrate plan --config apps/shoppr/platform.toml
coil migrate apply --config apps/shoppr/platform.toml --dry-run
coil migrate apply --config apps/shoppr/platform.toml --yes
```

Customer binary flow in the checked-in apps:

```bash
cd apps/shoppr
./scripts/prepare-local-dev.sh
cargo run -p shoppr -- migrate apply --dry-run
```

```bash
cd apps/gitly
./scripts/prepare-local-dev.sh
cargo run -p gitly -- migrate apply --dry-run
```

## What The Current Apps Prove

Shoppr and Gitly both already expose:

- migration contract counts during validation
- executable migration application reports
- manual customer migration entry reporting

That is a strong operator contract even where the public examples are still thinner than ideal on a
single dedicated customer-table migration walkthrough.

## Practical Migration Workflow

1. Validate the target release.
2. Run a dry migration apply or migration plan.
3. Review executable and manual migration entries.
4. Apply only when approved.
5. Record what changed.
6. Start the new release only after the expected migration state is confirmed.

## Customer-Specific Tables And Versioning

If customer-owned schema changes exist, treat them as first-class release work:

- version them with the customer app
- review them as part of release planning
- document rollback assumptions explicitly

Current public limitation:

- the repo currently exposes the reporting surface for manual customer migration entries
- but it does not yet provide a polished public docs example of a fully custom customer table
  migration end to end

So do not assume invisible customer schema changes are safe just because the runtime composes.

## Common Mistakes

### Applying migrations implicitly during startup

Startup should not be the first time operators learn whether migration state is safe.

### Ignoring manual migration entries

If the customer binary reports manual migration work, treat it as real release input.

### Forgetting rollback impact

Not every schema change is naturally reversible. Know the rollback posture before cutover.

## What To Read Next

- [Build and deploy](../build-and-deploy/)
- [Troubleshooting](../troubleshooting/)
- [Cache, TLS, cutover, and rollback](../cache-tls-cutover-and-rollback/)
