---
title: Project Organization
---

Davenda works best when the customer app, the runtime configuration, and any optional extension
lanes are structurally separate.

This is not just source-tree aesthetics. It is the operational boundary that keeps deploys,
troubleshooting, and ownership clear.

## What Is This?

This page describes how to organize a serious Davenda codebase so that:

- the customer binary is easy to find
- app manifests and platform config do not get mixed together
- linked customer Rust and bounded extensions stay distinct
- operators can tell what changed in a release

## Why Does This Matter?

When a Davenda repo is organized badly, the failure mode is predictable:

- startup and composition logic become hard to trace
- app identity and operational settings get mixed together
- customer-owned code starts looking like an undocumented plugin system
- deploy and rollback steps stop matching the source tree

Good project structure is one of the cheapest ways to keep the platform understandable.

## Recommended Top-Level Shape

A strong customer-root project should make these boundaries obvious:

- customer workspace crates
- app root files
- per-environment config
- auth package files
- templates and theme assets
- optional extension artifacts
- deployment and local-development scripts

The checked-in examples show two variants of that structure:

- `apps/shoppr/`
- `apps/gitly/`

## How Davenda Uses These Boundaries

In practice, a customer project usually splits into three layers.

### 1. Customer workspace crates

This is where the binary and customer-owned Rust logic live.

Concrete examples:

- `apps/shoppr/crates/shoppr-bin/`
- `apps/shoppr/crates/shoppr-app/`
- `apps/shoppr/crates/shoppr-backend/`
- `apps/gitly/crates/gitly-bin/`
- `apps/gitly/crates/gitly-app/`
- `apps/gitly/crates/gitly-backend/`

### 2. App root inputs

This is where the runtime-facing application inputs live.

Concrete examples:

- `apps/shoppr/app.toml`
- `apps/shoppr/platform.dev.toml`
- `apps/shoppr/platform.toml`
- `apps/shoppr/auth/shoppr-auth/`
- `apps/shoppr/templates/`
- `apps/shoppr/theme/`
- `apps/shoppr/extensions/`

The same pattern exists in Gitly under `apps/gitly/`.

### 3. Deployment and maintainer tooling

These files drive how the app runs locally or in containers.

Concrete examples:

- `apps/shoppr/docker-compose.yml`
- `apps/shoppr/docker-compose.repo.yml`
- `apps/shoppr/Dockerfile`
- `apps/shoppr/Dockerfile.repo`
- `apps/gitly/docker-compose.yml`
- `apps/gitly/Dockerfile`

## When To Use `davenda-all`

Use `davenda-all` when:

- you want the default official battery while learning
- you want the shortest path to a coherent full stack
- you are comfortable letting the app manifest decide which linked modules are actually enabled

Use narrower selective dependencies when:

- you are building a specialized product shape
- you want explicit control over what the binary links
- you need to explain the linked runtime surface to another team

The important rule is this: even when `davenda-all` is used, the customer binary still owns
composition.

## How To Add A New Customer Crate

For a new crate under a customer workspace:

1. Add a directory under `crates/`.
2. Add it to the customer workspace `Cargo.toml`.
3. Decide whether it is:
   - binary composition,
   - app/bootstrap logic,
   - customer backend logic,
   - or a shared support crate.
4. Wire it into the customer binary deliberately.

Good examples to imitate:

- `apps/shoppr/crates/shoppr-app/`
- `apps/gitly/crates/gitly-backend/`

## How To Add A Backend Crate

If you are adding customer-specific backend behavior:

1. Create a dedicated crate under `crates/`.
2. Keep the stable integration surface explicit through the customer SDK.
3. Register the backend from the customer binary or app composition crate.
4. Keep the backend crate focused on customer behavior, not generic platform code.

Concrete examples:

- `apps/shoppr/crates/shoppr-backend/`
- `apps/gitly/crates/gitly-backend/`

If the code genuinely needs its own process boundary, use a sidecar or external integration
instead of pretending it is still just linked backend logic.

## How To Add An Extension Folder

If you need bounded runtime-installed extensions:

1. Create an extension directory under `extensions/`.
2. Keep it separate from linked Rust crates.
3. Declare and pin it from the customer app manifest.
4. Treat it as a lower-trust or more replaceable boundary than linked customer Rust.

Concrete examples:

- `apps/shoppr/extensions/shoppr-waitlist-tools/`
- `apps/gitly/extensions/gitly-community-pulse/`
- `apps/gitly/extensions/gitly-actions-scheduler/`

## Working Pattern For Multi-Site Apps

For multi-site products, do not clone the app three times.

The Davenda shape is:

- one customer workspace
- one app manifest
- one shared deployment surface
- site-specific behavior expressed in app and platform config

Shoppr is the checked-in example of this model.

## Common Mistakes

### Mixing product and operations files together

`app.toml` and `platform.toml` should not become one ambiguous blob of settings.

### Hiding the app in helper scripts

If the only way to understand how the app starts is to read shell scripts, the customer binary is
too opaque.

### Treating linked Rust and extensions as the same lane

Linked customer Rust is first-party application logic. Runtime-installed extensions are not.

### Letting local maintainer overrides become the public model

Repo-maintainer conveniences such as Shoppr's repo override compose file are useful, but they are
not the customer-facing deployment contract.

## What To Read Next

- [Build and deploy](build-and-deploy.md)
- [Configuration and secrets](configuration-and-secrets.md)
- [Production topologies](production-topologies.md)
- [Customer project layout](../getting-started/customer-project-layout.md)
