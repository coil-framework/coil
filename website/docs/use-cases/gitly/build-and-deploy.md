---
title: Build And Deploy
---

Gitly proves that Davenda's operational model is not commerce-specific.

This page ties the general build and deploy guidance to the Gitly demo so developers building
non-commerce products can still see a canonical example.

## Repo Areas To Read

Start here:

- `apps/gitly/app.toml`
- `apps/gitly/platform.toml`
- `apps/gitly/platform.dev.toml`
- `apps/gitly/crates/gitly-bin/src/main.rs`
- `apps/gitly/crates/gitly-app/src/lib.rs`

## What The Build Looks Like

Gitly uses the same customer-root pattern as Shoppr:

- a customer app manifest
- platform config for development and production
- a customer binary
- a linked backend crate
- runtime-installed extensions

That is the point of the example. Davenda operations should look consistent across product shapes.

## Practical Local Flow

Typical local workflow:

```bash
cd apps/gitly
cargo run -p gitly-bin -- config validate --config platform.dev.toml
cargo run -p gitly-bin -- dev server --config platform.dev.toml
```

If your local environment uses object storage or database services, set the expected environment
variables from `platform.dev.toml` first.

## Production Flow

The production flow mirrors Shoppr:

1. validate config
2. plan and apply migrations
3. publish assets
4. inspect storage or queue state if needed
5. run release checks
6. perform cutover only when readiness is green

That flow is documented generically in [Build And Deploy](../../operations/build-and-deploy.md),
but Gitly shows that the same process works for a non-commerce app.

## Why This Matters

Without Gitly, it would be easy to assume Davenda's operations model only makes sense for commerce.

Gitly proves the opposite:

- the same config model works
- the same runtime build model works
- the same extension and linked backend story works
- the same deployment discipline works

## Read Next

- [Build And Deploy](../../operations/build-and-deploy.md)
- [Configuration And Secrets](../../operations/configuration-and-secrets.md)
- [Production Topologies](../../operations/production-topologies.md)
