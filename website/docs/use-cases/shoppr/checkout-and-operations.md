---
title: Checkout And Operations
---

This guide uses Shoppr to show how Davenda ties together checkout, account continuity, admin
surfaces, and operator workflows in one customer app.

## The Public Checkout Files

Read these templates in order:

1. `apps/shoppr/templates/commerce/cart.html`
2. `apps/shoppr/templates/commerce/checkout.html`
3. `apps/shoppr/templates/commerce/checkout-confirmation.html`

That sequence shows the public flow from basket review into payment handoff and confirmation.

## Where The Checkout Contract Comes From

The route contract comes from the commerce module manifest in
`crates/davenda-commerce/src/module/platform/manifest.rs`.

That module contributes:

- `/cart`
- `/checkout`
- `/checkout/start`
- `/checkout/complete`
- `/checkout/confirmation`
- `/webhooks/commerce/payment-provider`

Shoppr then supplies the product-specific templates and customer hook behavior.

## Stripe Handoff In Practice

Shoppr enables the Stripe provider module in:

- `apps/shoppr/app.toml`
- `apps/shoppr/platform.dev.toml`

And configures it in `platform.dev.toml` under:

- `[modules."commerce-payments-stripe"]`

This is a useful example because it shows the separation between:

- base commerce checkout
- provider-specific handoff and webhook config

## Account Continuity After Checkout

The post-checkout customer story continues in:

- `apps/shoppr/templates/pages/account.html`
- `apps/shoppr/templates/account/dashboard.html`
- `apps/shoppr/templates/account/orders.html`
- `apps/shoppr/templates/account/summary-panels.html`
- `apps/shoppr/templates/memberships/account.html`

These files matter because they keep the app honest about what happens after provider return:

- pending payment can still be pending
- order history is part of the account flow
- membership activation is a follow-on lifecycle, not just a marketing claim

## Where First-Party Policy Lives

Shoppr's first-party store logic lives in linked Rust:

- `apps/shoppr/crates/shoppr-backend/src/lib.rs`
- `apps/shoppr/backend/shoppr-loyalty-backend/src/lib.rs`

That is where checkout review and verified-webhook behavior are owned.

If you are building your own store, this is the boundary to study for:

- customer-specific order review
- loyalty rules
- CRM routing
- webhook follow-up

## Where Bounded Extensions Live

Shoppr also keeps a bounded WASM path in:

- `apps/shoppr/extensions/shoppr-waitlist-tools/package.toml`
- `apps/shoppr/crates/shoppr-app/src/extensions.rs`

That gives you one app that shows both extension models without confusing them.

## Operator And Support Surfaces

Day-one store operations are visible in:

- `apps/shoppr/templates/admin/dashboard.html`
- `apps/shoppr/templates/admin/audit.html`
- `apps/shoppr/templates/commerce/orders.html`
- `apps/shoppr/templates/commerce/order-detail.html`
- `apps/shoppr/templates/commerce/catalog-admin.html`

These pages show the operator side of the same store:

- order queue and detail
- refund and fulfillment flow
- catalog copy and visibility management
- audit and admin shell

That is a strong Davenda lesson: the customer app owns the operator story too.

## Lifecycle And Runtime Touchpoints

Shoppr's customer-owned lifecycle is visible in:

- `apps/shoppr/crates/shoppr-app/src/lib.rs`
- `apps/shoppr/crates/shoppr-bin/src/main.rs`
- `apps/shoppr/platform.dev.toml`

That layer owns:

- manifest and config loading
- auth package loading
- official module resolution
- linked plugin registration
- extension package loading
- validate, migrate, assets, serve, and up commands

## Adapt This For Your Store

If you are using Shoppr as a starting point, keep these patterns:

- public cart and checkout flow in templates
- provider config in a separate payment module block
- post-checkout truthfulness in account templates
- linked Rust for first-party policy
- operator pages in the same customer app

## Read Next

- [Linked Rust Backend](./linked-rust-backend.md)
- [WASM Extensions](./wasm-extensions.md)
- [Commerce Module Reference](../../reference/modules/commerce.md)
