---
title: Checkout And Operations
---

This page is about Davenda’s checkout and operator model first, with Shoppr as a concrete example.

Use it when you want to answer:

- where the public checkout contract comes from
- where payment-provider boundaries live
- how account continuity and operator visibility fit into the same app
- which files to copy when building a real store

## The Core Pattern

Davenda keeps the commerce lifecycle split across four layers:

1. a reusable route and handler contract from the commerce module
2. customer templates for cart, checkout, confirmation, account, and admin surfaces
3. customer-owned payment-provider configuration
4. customer-owned policy in linked Rust where the store needs custom decisions

That is the right mental model to keep while reading Shoppr. The demo is evidence of the pattern,
not the pattern itself.

## Canonical Commerce Route Contract

The route contract comes from `crates/davenda-commerce/src/module/platform/manifest.rs`.

The important part is that the module contributes named surfaces like:

```rust
"/cart"
"/checkout"
"/checkout/start"
"/checkout/complete"
"/checkout/confirmation"
"/webhooks/commerce/payment-provider"
```

That snippet matters because it shows the reusable platform boundary:

- cart and checkout are module-owned route contracts
- the provider webhook is part of the same commerce lifecycle
- the customer app does not need to invent the route vocabulary first

Shoppr then supplies the implementation-facing pieces around that contract.

## Canonical Provider Configuration Shape

The customer app keeps payment-provider configuration in platform config, not inside templates.

Shoppr’s local example in `apps/shoppr/platform.dev.toml` uses:

```toml
[modules."commerce-payments-stripe"]
provider = "stripe"
checkout_mode = "hosted-checkout"
publishable_key = { kind = "env", var = "STRIPE_PUBLISHABLE_KEY" }
webhook_secret = { kind = "env", var = "STRIPE_WEBHOOK_SECRET" }
```

That is the pattern to copy:

- provider identity and mode stay in config
- secrets stay as env-backed secret refs
- the public checkout template does not become the source of payment truth

## Canonical Customer Lifecycle Commands

The customer binary should own the app lifecycle for developers and operators.

Shoppr’s binary entrypoint in `apps/shoppr/crates/shoppr-bin/src/main.rs` exposes:

```rust
enum Command {
    Describe,
    Validate,
    Assets { command: AssetsCommand },
    Migrate { command: MigrateCommand },
    Serve { bind: Option<String> },
    Up { bind: Option<String> },
    LinkedBackend { command: LinkedBackendCommand },
}
```

This is the right operational shape:

- the root platform CLI still exists for global operator workflows
- the customer app owns the app-specific lifecycle a new developer actually runs

## What The Customer App Still Owns

The customer app owns the human product surfaces around the reusable module contract.

For Shoppr, that means:

- cart and checkout templates
- confirmation and account continuity
- order-support and admin pages
- any custom review or webhook policy in linked Rust

## Shoppr As The Supporting Example

### Public checkout

Shoppr’s public checkout templates live in:

- `apps/shoppr/templates/commerce/cart.html`
- `apps/shoppr/templates/commerce/checkout.html`
- `apps/shoppr/templates/commerce/checkout-confirmation.html`

Those are useful because they show the public flow without confusing the source of truth:

- cart is still an SSR surface
- checkout explains hosted-provider handoff honestly
- confirmation does not overclaim settlement or membership activation

### Account continuity

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

### First-party policy

Shoppr’s linked Rust boundary lives in:

- `apps/shoppr/crates/shoppr-backend/src/lib.rs`
- `apps/shoppr/backend/shoppr-loyalty-backend/src/lib.rs`

This is where customer-specific order review, loyalty, CRM, and verified-webhook decisions belong.

### Operator surfaces

Day-one store operations are visible in:

- `apps/shoppr/templates/admin/dashboard.html`
- `apps/shoppr/templates/admin/audit.html`
- `apps/shoppr/templates/commerce/orders.html`
- `apps/shoppr/templates/commerce/order-detail.html`
- `apps/shoppr/templates/commerce/catalog-admin.html`

These pages show the operator side of the same lifecycle:

- order queue and detail
- refund and fulfillment visibility
- catalog copy and visibility changes
- audit and admin shell

## Practical Rules To Copy

- let the commerce module define the core route contract
- keep provider selection and secrets in config
- keep public checkout templates honest about settlement and return flow
- keep account continuity in the same customer app
- put customer-specific review and webhook policy in linked Rust
- make operator pages part of the same product, not an afterthought

## Full Implementation Pointers

If you want the full Shoppr implementation after reading the pattern:

- `apps/shoppr/templates/commerce/cart.html`
- `apps/shoppr/templates/commerce/checkout.html`
- `apps/shoppr/templates/commerce/checkout-confirmation.html`
- `apps/shoppr/templates/pages/account.html`
- `apps/shoppr/templates/account/dashboard.html`
- `apps/shoppr/templates/account/orders.html`
- `apps/shoppr/templates/memberships/account.html`
- `apps/shoppr/crates/shoppr-backend/src/lib.rs`
- `apps/shoppr/backend/shoppr-loyalty-backend/src/lib.rs`
- `apps/shoppr/crates/shoppr-app/src/lib.rs`
- `apps/shoppr/crates/shoppr-bin/src/main.rs`

## Read Next

- [Linked Rust Backend](./linked-rust-backend.md)
- [Jobs, Webhooks, And Background Work](./jobs-webhooks-and-background-work.md)
- [Commerce Module Reference](../../reference/modules/commerce.md)
