---
title: Project Organization
---

Davenda works best when the customer app, the runtime configuration, and any optional extension
lanes are structurally separate.

This is not just source-tree aesthetics. It is the operational boundary that keeps deploys,
troubleshooting, and ownership clear.

## What Is This?

This page explains how to lay out a serious Davenda codebase so that:

- the customer binary is obvious
- product and operational inputs are not mixed together
- linked customer Rust and bounded extensions stay distinct
- release inputs are easy to identify

## Why Does This Matter?

Bad structure causes predictable operational failures:

- startup and composition logic become hard to trace
- app identity and runtime topology get mixed together
- customer-owned code starts looking like an undocumented plugin system
- deploy and rollback steps stop matching the repository

Good structure is one of the cheapest ways to keep a Davenda app operable.

## The Canonical Shape

A healthy Davenda customer project usually has four clear areas:

1. a Rust workspace that owns the customer binary and customer-owned crates
2. an app root that owns `app.toml`, templates, theme assets, auth files, and extensions
3. per-environment runtime config such as `platform.dev.toml` and `platform.toml`
4. deployment tooling such as container files and scripts

That shape matters more than the exact folder names.

## A Practical Layout

This is a good starting point:

```text
customer-product/
  Cargo.toml
  crates/
    product-bin/
    product-app/
    product-backend/
  app.toml
  platform.dev.toml
  platform.toml
  auth/
  templates/
  theme/
  extensions/
  docker-compose.yml
  Dockerfile
```

What each area is for:

- `product-bin/`: the composition root and operator-facing binary
- `product-app/`: app/bootstrap logic and customer-specific runtime assembly
- `product-backend/`: linked customer Rust behaviour
- `app.toml`: product composition and app identity
- `platform*.toml`: environment-specific runtime topology
- `auth/`: customer auth package files
- `templates/` and `theme/`: frontend presentation
- `extensions/`: bounded runtime-installed packages

## What Belongs Where

### Put this in the Rust workspace

- binary composition
- customer-specific backend logic
- any shared customer support crates

### Put this in the app root

- app identity
- site and locale structure
- enabled modules
- templates and theme assets
- auth package files
- extension package declarations

### Put this in runtime config

- bind addresses
- database, cache, jobs, and storage backends
- TLS mode
- observability settings
- CDN and asset delivery settings

### Put this in secret storage

- database URLs
- provider API keys
- webhook secrets
- object-store credentials

## When To Use `davenda-all`

Use `davenda-all` when:

- you want the default official battery while learning
- you want the shortest path to a coherent full stack
- you are comfortable letting the app manifest decide which linked modules are actually enabled

Use narrower selective dependencies when:

- you are building a specialized product shape
- you want tighter control over what the binary links
- you need a very explicit runtime surface for review or compliance

Even when `davenda-all` is used, the customer binary still owns composition.

## How To Add A New Customer Crate

When adding a crate to a customer workspace:

1. decide whether it is binary composition, app/bootstrap logic, customer backend logic, or shared support code
2. add it to the customer workspace manifest
3. link it intentionally from the customer binary or app crate
4. keep its responsibility narrow enough that another operator can tell why it exists

Good rule:

- if the crate owns product-specific behaviour, it belongs in customer code
- if the crate is becoming generic enough for many apps, reevaluate whether it wants to be an official module instead

## How To Add A Backend Crate

If you need customer-specific backend behaviour:

1. create a dedicated backend crate
2. implement supported customer SDK hooks or facades there
3. register it from the customer binary or app composition crate
4. keep it separate from lower-trust runtime-installed extensions

If the code genuinely needs its own process boundary, use a sidecar or external integration on
purpose. Do not create a fake service boundary by habit.

## How To Add An Extension Directory

If you need bounded runtime-installed extensions:

1. create an `extensions/` area
2. keep extension code separate from linked customer Rust
3. declare and pin the extension from the app manifest
4. treat it as a replaceable, lower-trust boundary

The important distinction is:

- linked customer Rust is part of the app
- runtime-installed extensions are explicitly bounded add-ons

## Multi-Site Project Organization

For multi-site products, do not clone the application three times unless there is a real product
boundary that requires it.

The Davenda shape is:

- one customer workspace
- one app manifest
- one deployment surface
- site-specific behaviour expressed in config and content

That keeps rollout, auth, jobs, and operations coherent.

## Supporting Repo Examples

If you want concrete examples after reading the pattern:

- Shoppr shows the multi-site commerce shape
- Gitly shows the single-site non-commerce shape

Those examples are useful because they prove the model, but they are not the primary teaching
material for this page.

## Common Mistakes

### Mixing product and operational inputs

`app.toml` and `platform.toml` should not become one ambiguous blob of settings.

### Hiding composition in scripts

If the only way to understand startup is to read shell wrappers, the customer binary is too opaque.

### Treating linked Rust and extensions as one lane

They exist for different trust and ownership models.

### Letting local maintainer overrides become the public contract

Repo-maintainer conveniences are useful, but they are not the supported customer deployment model.

## What To Read Next

- [Configuration and secrets](configuration-and-secrets.md)
- [Production topologies](production-topologies.md)
- [Build and deploy](build-and-deploy.md)
- [Customer project layout](../getting-started/customer-project-layout.md)
