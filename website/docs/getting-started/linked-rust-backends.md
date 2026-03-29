---
title: Linked Rust Backends
---

Coil's preferred customization model is linked customer Rust.

That means customer-specific backend behaviour lives in ordinary Rust crates that are compiled into the customer application, not in a separate API service by default.

## How To Use This Page

Use this page as the launch point for the customer-owned backend lane.

- Read it if you want to know when Coil expects linked Rust instead of an external service.
- From here, jump into the customer-root workspace model, the reference boundary between customer
  Rust and WASM, and the checked-in Shoppr and Gitly examples.
- If you are already convinced and just need the app shape, go back to
  [Customer project layout](customer-project-layout.md).

## What It Is

A linked Rust backend is customer-owned code that plugs into Coil through supported public APIs and hook/facade boundaries.

Typical responsibilities include:

- product-specific checkout rules
- CMS publish validation
- request-time page and block shaping through render-model hooks
- verified webhook handling
- customer-specific admin or integration behaviour

The exact extension points are intentionally explicit. Coil does not expose the whole runtime as an ambient bag of internals.

## Why It Exists

Coil makes this the primary customization path because it keeps the application honest.

### You get compile-time integration

Your product logic is built, typed, and tested together with the application instead of floating in a separate service boundary.

### You avoid unnecessary infrastructure

Many teams default to "add another API" because their framework has no good first-party customization path. Coil is trying to remove that pressure.

### You keep trust boundaries explicit

Customer-owned Rust has a different trust model from third-party extensions. Coil treats those as different things on purpose.

## How It Works

At a high level:

1. The customer binary links Coil crates plus customer-owned crates.
2. The customer backend implements supported plugin or hook traits.
3. The runtime exposes stable facades instead of leaking arbitrary internals.
4. Request-time or lifecycle-time hooks are invoked through those public surfaces.

This model is strong enough to let customer code participate in first-party behaviour while still preserving a stable runtime boundary.

## Where To See This In Practice

Use these pages together rather than in isolation:

- [Customer-root workspace](../core-concepts/customer-root-workspace.md)
- [Render Model Hooks](../reference/render-model-hooks.md)
- [Customer Rust vs third-party WASM](../reference/customer-vs-wasm.md)
- [Shoppr overview](../use-cases/shoppr/overview.md)
- [Gitly overview](../use-cases/gitly/overview.md)

Operationally relevant follow-ons:

- [Jobs and schedulers](../operations/jobs-and-schedulers.md)
- [Observability, monitoring, and audit](../operations/observability.md)
- [Webhooks and integrations](../operations/webhooks-and-integrations.md)

## When Not To Use It

Do not force everything into linked Rust.

You may still want:

- a separate service when the boundary is operationally real
- a third-party WASM extension when the code is lower-trust or marketplace-style
- plain HTTP integration when the dependency should remain external

The point is not "everything must be linked." The point is that Coil has a clear default path when the code is truly customer-owned application logic.

## One Important Current Capability

If you need customer-owned server-side page shaping, use linked Rust render-model hooks.

That is the supported path for:

- mounting your own top-level model prefix such as `crm_page`
- merging fields into `page`
- shaping block-driven pages at request time before template render

Read [Render Model Hooks](../reference/render-model-hooks.md) for the exact API and merge rules.

## Common Mistakes

### Recreating a sidecar by habit

If the code is product-specific and needs first-party access to application behaviour, starting with an external service is usually the wrong default.

### Expecting runtime internals as the API

The supported contract is the customer SDK and stable facades, not direct access to every internal runtime type.

### Confusing customer Rust with third-party extensions

Linked customer Rust and bounded WASM extensions serve different goals and operate under different trust assumptions.

## What To Read Next

- [Customer-root workspace](../core-concepts/customer-root-workspace.md)
- [Customer apps vs official modules](../core-concepts/customer-apps-vs-official-modules.md)
- [Customer Rust vs third-party WASM](../reference/customer-vs-wasm.md)
- [Shoppr overview](../use-cases/shoppr/overview.md)
- [Gitly overview](../use-cases/gitly/overview.md)
- [Observability, monitoring, and audit](../operations/observability.md)
