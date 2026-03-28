---
title: Asset Publication And CDN Delivery
---

Davenda treats asset publication as a release step, not a side effect of rendering.

## What Is This?

This page explains how theme and managed assets move from the customer app into a deployable
published state, and how `cdn_base_url` affects delivery.

## Why Does This Matter?

A correct binary can still serve a broken frontend if:

- assets were not published
- the manifest is stale
- the CDN path is wrong
- the runtime and asset release are out of sync

That is why asset publication is a first-class operator workflow.

## Concrete Repo Examples

Shoppr local development:

- `apps/shoppr/platform.dev.toml`
- `cdn_base_url = "http://localhost:9000/shoppr"`

Gitly local development:

- `apps/gitly/platform.dev.toml`
- `cdn_base_url = "http://localhost:9002/gitly"`

Shoppr production-shaped config:

- `apps/shoppr/platform.toml`
- `cdn_base_url = "https://cdn.example.com"`

## The Publication Command

Concrete customer-binary example:

```bash
cd apps/shoppr
./scripts/prepare-local-dev.sh
cargo run -p shoppr -- assets publish
```

Generic platform example:

```bash
platform assets publish --config apps/shoppr/platform.toml --dry-run
```

## Same-Origin Versus CDN Delivery

### Same-origin delivery

Use it when:

- you want the simplest deployment shape
- you are still validating the product
- a dedicated CDN is not yet worth the complexity

### CDN delivery

Use it when:

- you want stronger frontend caching
- you want asset delivery to scale independently
- you want to keep asset traffic off the app origin

If you use a CDN, asset publication becomes a hard release dependency.

## What The Runtime Needs

For asset delivery to work safely, operators need:

- a published asset manifest
- a stable `cdn_base_url` strategy
- predictable object-store or CDN credentials
- a release record that ties binary and assets together

## Local Development Reality

The checked-in apps already prove this lane locally:

- Shoppr and Gitly publish assets into local MinIO-backed flows
- their Docker stacks wire `cdn_base_url` accordingly

That makes local asset behavior close enough to production to catch real mistakes.

## Common Mistakes

### Treating asset publication as optional

If the app expects published hashed assets, publication is part of release, not an afterthought.

### Changing `cdn_base_url` without release coordination

That can break public pages even if the runtime is otherwise healthy.

### Assuming local-only asset paths prove production correctness

Production CDN behavior still needs deliberate validation.

## What To Read Next

- [Build and deploy](build-and-deploy.md)
- [Configuration and secrets](configuration-and-secrets.md)
- [Cache, TLS, cutover, and rollback](cache-tls-cutover-and-rollback.md)
