---
title: Linked Rust Backend
---

Shoppr is the main example of first-party customer logic compiled directly into the customer app.

This page shows which files to read and what each layer is responsible for.

## The Two Backend Layers

Shoppr keeps the linked backend split into two crates:

- `apps/shoppr/crates/shoppr-backend`
- `apps/shoppr/backend/shoppr-loyalty-backend`

That split is deliberate.

- `shoppr-backend` is the Davenda-facing plugin crate.
- `shoppr-loyalty-backend` is the customer domain library with the real store rules.

If you are building your own app, this is a strong pattern to copy.

## What The Davenda-Facing Plugin Does

Read `apps/shoppr/crates/shoppr-backend/src/lib.rs`.

That crate:

- defines the linked plugin descriptor
- registers checkout hooks
- registers verified-webhook hooks
- publishes a stable plugin summary for the customer binary and docs

This is the seam between Davenda and customer code.

## What The Customer Domain Library Does

Read `apps/shoppr/backend/shoppr-loyalty-backend/src/lib.rs`.

That crate contains the customer logic itself, such as:

- order review policy
- loyalty preview logic
- CRM routing decisions

This is important because it shows the right separation:

- SDK-facing glue in one crate
- business rules in another crate

## Where The Plugin Is Wired Into The App

The plugin becomes live in `apps/shoppr/crates/shoppr-app/src/lib.rs`.

Look for the customer plugin vector:

- `vec![Box::new(shoppr_backend::plugin())]`

That is the customer-root composition moment. The plugin is not discovered by magic or loaded by a
global registry. The customer app chooses it explicitly.

## What Runtime Flows It Participates In

Today Shoppr uses linked Rust for:

- checkout review
- verified payment webhook handling
- plugin summary and lifecycle reporting

You can see those responsibilities in the traits implemented by `ShopprBackend` in
`apps/shoppr/crates/shoppr-backend/src/lib.rs`.

## Why This Is The Primary Customization Path

Shoppr is a good example because it makes the linked Rust path feel normal:

- it lives in the customer workspace
- it ships with the app
- it uses stable SDK traits
- it stays inside the same deployment and release path

That is exactly what Davenda wants for first-party customer business logic.

## Adapt This For Your Own App

Copy this structure:

1. one small plugin crate that implements Davenda traits
2. one domain library crate that contains the real customer rules
3. explicit plugin injection from the customer composition root

Do not start with a sidecar if the boundary is not operationally real.

## Read Next

- [Customer Rust Vs Third-Party WASM](../../reference/customer-vs-wasm.md)
- [Shoppr WASM Extensions](./wasm-extensions.md)
- [Shoppr Checkout And Operations](./checkout-and-operations.md)
