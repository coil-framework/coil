---
title: Official Modules
---

Davenda's official modules are reusable product batteries. They sit above core and below customer
apps.

Use this page as the entry point when you want to answer practical questions such as:

- which module owns a workflow
- which capabilities a module expects
- which routes or operator surfaces it adds
- which demo app already uses it

## What A Module Is

An official module packages one reusable business domain. In practice that usually means some
combination of:

- capability requirements
- migrations
- public and admin routes
- jobs and event subscriptions
- admin resources
- search or reporting contributions
- extension slots for customer apps

The clearest source of truth is always the module manifest in the relevant crate:

- `crates/davenda-cms/src/module/platform/manifest.rs`
- `crates/davenda-media/src/module/manifest.rs`
- `crates/davenda-commerce/src/module/platform/manifest.rs`
- `crates/davenda-memberships/src/module/manifest.rs`
- `crates/davenda-events/src/module/platform/manifest.rs`
- `crates/davenda-admin/src/module/manifest.rs`
- `crates/davenda-ops/src/module/manifest.rs`

## How Modules Are Installed

Davenda has two different decisions:

1. The customer binary links module crates at compile time.
2. The customer app manifest and platform config enable a subset of those linked modules at
   runtime.

That is why Shoppr can link a broad stack in `apps/shoppr/Cargo.toml` and still decide the real
product surface through `apps/shoppr/app.toml` and `apps/shoppr/platform.dev.toml`.

## Quick Module Map

| Module | Owns | Good demo |
| --- | --- | --- |
| CMS | pages, navigation, redirects, preview, publish workflow | Shoppr |
| Media | managed assets, media library, storage policy UI | Shoppr |
| Commerce | catalog, cart, checkout, orders | Shoppr |
| Commerce Payments Stripe | Stripe handoff and signed webhook reconciliation | Shoppr |
| Memberships | tiers, subscriptions, account memberships | Shoppr |
| Events | event catalog, bookings, reminders, check-in | Shoppr |
| Admin | shared admin shell and audit entry surfaces | Shoppr, Gitly |
| Ops | search, reports, recovery, bulk operations | Shoppr |

## Module Guides

- [CMS](./modules/cms.md)
- [Media](./modules/media.md)
- [Commerce](./modules/commerce.md)
- [Commerce Payments Stripe](./modules/commerce-payments-stripe.md)
- [Memberships](./modules/memberships.md)
- [Events](./modules/events.md)
- [Admin](./modules/admin.md)
- [Ops](./modules/ops.md)

## Choosing Between A Module And Customer Code

Use an official module when:

- the behavior is reusable across more than one product
- the platform should support it as a stable contract
- it needs shared migrations, auth, jobs, and operator surfaces

Keep the behavior in customer code when:

- it is product-specific policy
- it only makes sense for one app
- it is better expressed as linked Rust or a bounded extension hook

Shoppr and Gitly are good examples of that split:

- Shoppr uses official modules for CMS, commerce, memberships, events, admin, and ops, but keeps
  loyalty rules in `apps/shoppr/crates/shoppr-backend/src/lib.rs`.
- Gitly uses `admin`, `cms`, and `media`, then adds its own non-commerce product shell in
  `apps/gitly/crates/gitly-app/src/lib.rs`.

## Common Mistakes

- Treating modules as compile-time features only. Runtime enablement still matters.
- Enabling a module in `app.toml` without linking it into the customer binary.
- Treating a module as “just routes.” The migrations, jobs, and capabilities usually matter as
  much as the pages.
- Putting one customer's business rules into a new official module too early.

## Read Next

- [Composition And `davenda-all`](./composition.md)
- [Customer Rust Vs Third-Party WASM](./customer-vs-wasm.md)
- [Shoppr Overview](../use-cases/shoppr/overview.md)
- [Gitly Overview](../use-cases/gitly/overview.md)
