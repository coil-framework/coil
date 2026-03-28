---
title: Commerce Payments Stripe
---

`commerce-payments-stripe` is the Stripe provider module that extends the base commerce checkout
flow with hosted checkout handoff and signed webhook reconciliation.

Primary implementation files:

- `crates/davenda-commerce/src/module/stripe.rs`
- `apps/shoppr/platform.dev.toml`

## Why It Exists

Davenda keeps payment-provider integration separate from the base commerce module so a customer app
can choose:

- no provider yet
- Stripe now
- a different provider later

without collapsing all provider semantics into the core catalog and order model.

## What It Provides

The Stripe module manifest in `crates/davenda-commerce/src/module/stripe.rs` contributes:

- module id `commerce-payments-stripe`
- config namespace `commerce_payments_stripe`
- a required dependency on `commerce`
- Stripe provider metadata such as checkout mode and webhook route
- the payment webhook route constant `/webhooks/commerce/payment-provider`

## How To Enable It

Enable both base commerce and the Stripe module:

```toml title="app.toml"
[modules]
enabled = ["commerce", "commerce-payments-stripe"]
```

Then configure the provider in platform config:

```toml title="platform.dev.toml"
[modules]
enabled = ["commerce", "commerce-payments-stripe"]

[modules."commerce-payments-stripe"]
provider = "stripe"
checkout_mode = "hosted-checkout"
publishable_key = { kind = "env", var = "STRIPE_PUBLISHABLE_KEY" }
webhook_secret = { kind = "env", var = "STRIPE_WEBHOOK_SECRET" }
```

Shoppr uses that exact shape in `apps/shoppr/platform.dev.toml`.

## How To Disable It

Remove `commerce-payments-stripe` from the module lists and remove or replace the Stripe-specific
settings block. The base commerce module can remain enabled.

## Config Expectations

The checked-in Stripe path expects:

- `provider = "stripe"`
- `checkout_mode`
- `publishable_key`
- `webhook_secret`

The runtime also expects the secret key to be available through a secret binding. Shoppr shows that
under `[wasm.secret_bindings]` in `apps/shoppr/platform.dev.toml`.

## Routes And Surfaces

The important surface is the webhook contract:

- `/webhooks/commerce/payment-provider`

The public checkout and confirmation pages still come from the base commerce module. Stripe extends
that flow; it does not replace the whole storefront.

## Required Auth Capabilities

This add-on module does not declare a large new capability set. It extends commerce through module
dependency and provider configuration rather than new storefront permissions.

## How Customer Apps Extend It

Customer apps usually extend the Stripe path through:

- linked verified-webhook hooks
- operator-facing order detail pages
- customer-facing confirmation and account templates

Shoppr demonstrates all three.

## Where To See It

- `apps/shoppr/platform.dev.toml`
- `apps/shoppr/templates/commerce/checkout.html`
- `apps/shoppr/templates/commerce/checkout-confirmation.html`
- `apps/shoppr/templates/commerce/order-detail.html`

## Common Mistakes

- Enabling the Stripe module without enabling `commerce`.
- Forgetting to provide `publishable_key` and `webhook_secret`.
- Treating the hosted checkout handoff as sufficient without the signed webhook path.

## Read Next

- [Commerce](./commerce.md)
- [Shoppr Checkout And Operations](../../use-cases/shoppr/checkout-and-operations.md)
