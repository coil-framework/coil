---
title: Asset Publication And CDN Delivery
---

Coil treats asset publication as a release step, not a side effect of rendering.

## What Is This?

This page explains how theme and managed assets move from the customer app into a deployable
published state, and how `cdn_base_url` affects delivery.

## Why Does This Matter?

A correct binary can still serve a broken frontend if:

- assets were not published
- the asset manifest is stale
- the CDN path is wrong
- the runtime and asset release are out of sync

That is why asset publication is a first-class operator workflow.

## The Canonical Delivery Model

Think about asset delivery as three steps:

1. build or prepare the asset set that belongs to the release
2. publish those assets and the manifest that maps logical names to release outputs
3. point the runtime at the correct delivery base URL

If any of those three steps drift, the public frontend can break even while the backend stays
healthy.

## Publication Command Surface

The operator shape should be explicit:

```bash
coil assets publish --config config/platform.toml --dry-run
coil assets publish --config config/platform.toml --yes
```

Customer binaries may re-export the same lifecycle, for example:

```bash
cargo run -p shoppr -- assets publish
```

## Same-Origin Versus CDN Delivery

### Same-origin delivery

Use it when:

- you want the simplest deployment shape
- you are still validating the product
- a dedicated CDN is not yet worth the operational cost

### CDN delivery

Use it when:

- you want stronger frontend caching
- you want asset delivery to scale independently
- you want to keep asset traffic off the app origin

If you use a CDN, asset publication becomes a hard dependency of release promotion.

## What The Runtime Needs

For asset delivery to work safely, operators need:

- a published asset manifest
- a stable `cdn_base_url` strategy
- predictable object-store or CDN credentials
- a release record that ties binary and asset state together

## A Practical Config Example

Development example:

```toml
[assets]
publish_manifest = true
cdn_base_url = "http://localhost:9000/app"
```

Production example:

```toml
[assets]
publish_manifest = true
cdn_base_url = "https://cdn.example.com"
```

The important rule is not the exact URL. It is that the configured delivery base must match the
published asset state for the release you are rolling out.

## Local Development Reality

Good local setups still exercise the asset publication path rather than bypassing it entirely.

That is useful because it catches:

- stale manifest assumptions
- bad asset paths
- missing object-store or local CDN wiring

## Supporting Repo Examples

The checked-in apps prove this lane with local object-store-backed asset delivery:

- Shoppr uses a local dev `cdn_base_url` and publishes assets through the customer binary
- Gitly does the same with a different local port and asset prefix

Those examples are worth reading after this page if you need a concrete implementation, but the
delivery model above is the main thing to understand first.

## Common Mistakes

### Treating asset publication as optional

If the app expects hashed published assets, publication is part of release, not an afterthought.

### Changing `cdn_base_url` without release coordination

That can break public pages even if the runtime is healthy.

### Assuming local-only paths prove production correctness

Production CDN behaviour still needs deliberate validation and rollback planning.

## What To Read Next

- [Build and deploy](build-and-deploy.md)
- [Configuration and secrets](configuration-and-secrets.md)
- [Cache, TLS, cutover, and rollback](cache-tls-cutover-and-rollback.md)
