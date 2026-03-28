---
title: Shoppr Overview
---

Shoppr is the main Davenda commerce teaching app. It is useful because it shows the whole customer
product boundary in one place:

- manifest and config
- linked Rust backend
- runtime-installed WASM
- storefront templates
- account and memberships
- admin and operator pages
- customer-owned lifecycle commands

If you want to understand how Davenda is meant to feel in a real ecommerce app, start here.

## What Shoppr Is

The checked-in app lives under `apps/shoppr/`. Read these files first:

1. `apps/shoppr/app.toml`
2. `apps/shoppr/platform.dev.toml`
3. `apps/shoppr/crates/shoppr-app/src/lib.rs`
4. `apps/shoppr/crates/shoppr-bin/src/main.rs`

That sequence answers four different questions:

- what the product is
- how the local runtime is configured
- how the customer runtime plan is composed
- how the customer binary owns validate, migrate, assets, and serve commands

## What Shoppr Enables

The app manifest enables a broad but realistic store stack:

- `cms`
- `media`
- `commerce`
- `commerce-payments-stripe`
- `memberships`
- `events`
- `admin`
- `ops`

That list is not decorative. It tells you immediately what product batteries Shoppr is teaching:

- editorial pages and redirects
- managed assets
- catalog and checkout
- Stripe handoff and webhook reconciliation
- recurring memberships
- event flows
- operator shell and ops surfaces

## How The Workspace Is Structured

Shoppr uses a customer-root workspace, not a single crate.

Important folders:

- `apps/shoppr/crates/shoppr-app`
  - customer composition root
  - loads manifest, config, auth package, official modules, and extensions
- `apps/shoppr/crates/shoppr-bin`
  - customer-owned CLI and server entrypoint
- `apps/shoppr/crates/shoppr-backend`
  - Davenda-facing linked plugin wrapper
- `apps/shoppr/backend/shoppr-loyalty-backend`
  - customer domain logic used by the linked plugin
- `apps/shoppr/extensions`
  - runtime-installed WASM packages
- `apps/shoppr/templates`
  - storefront, account, CMS, admin, and operator templates
- `apps/shoppr/theme`
  - CSS, JS, SVG, and tokens

That structure is the first big lesson. Davenda customer apps are real products with their own
workspace, not just a folder full of overrides.

## What To Read In The App

### Storefront and merchandising

Read:

- `apps/shoppr/templates/pages/home.html`
- `apps/shoppr/templates/commerce/catalog.html`
- `apps/shoppr/templates/commerce/collection-detail.html`
- `apps/shoppr/templates/commerce/product-detail.html`

These files show how the customer app owns the browse loop directly.

### Cart, checkout, and confirmation

Read:

- `apps/shoppr/templates/commerce/cart.html`
- `apps/shoppr/templates/commerce/checkout.html`
- `apps/shoppr/templates/commerce/checkout-confirmation.html`

These files show the public checkout path without pretending the runtime is a generic SPA shell.

### Account, memberships, and order continuity

Read:

- `apps/shoppr/templates/pages/account.html`
- `apps/shoppr/templates/account/dashboard.html`
- `apps/shoppr/templates/account/orders.html`
- `apps/shoppr/templates/memberships/account.html`

These files show what the customer sees after checkout and provider return.

### Admin and operations

Read:

- `apps/shoppr/templates/admin/dashboard.html`
- `apps/shoppr/templates/admin/audit.html`
- `apps/shoppr/templates/commerce/orders.html`
- `apps/shoppr/templates/commerce/order-detail.html`
- `apps/shoppr/templates/commerce/catalog-admin.html`
- `apps/shoppr/templates/cms/pages.html`

These files show what the store operator owns on day one.

## Sites, Locales, And Theme Ownership

Shoppr is also the canonical multi-site commerce demo.

`apps/shoppr/app.toml` declares:

- canonical and additional domains
- app-level i18n settings
- three sites: UK, France, and Poland
- site-specific default locales and brand names

`apps/shoppr/platform.dev.toml` mirrors those sites for runtime host resolution.

The theme then makes those choices visible through:

- `apps/shoppr/theme/assets/site.css`
- `apps/shoppr/theme/assets/site.js`
- `apps/shoppr/theme/tokens.toml`

That is the practical Davenda story: site policy, locale policy, and theme behavior all live in
the customer app.

## Linked Rust And WASM In One Commerce App

Shoppr demonstrates both extension models clearly.

Linked Rust:

- `apps/shoppr/crates/shoppr-backend/src/lib.rs`
- `apps/shoppr/backend/shoppr-loyalty-backend/src/lib.rs`

WASM:

- `apps/shoppr/extensions/shoppr-waitlist-tools/package.toml`
- `apps/shoppr/crates/shoppr-app/src/extensions.rs`

Use Shoppr when you want to understand where first-party logic stops being “config” and becomes
linked code or a bounded extension.

## Adapt This For Your Store

If you are building a Davenda store, copy these ideas before copying markup:

- keep the customer workspace explicit
- let `app.toml` define the product contract
- keep market and locale policy in manifest and config
- own the full browse, account, and operator journey in one app
- use linked Rust for first-party store policy
- use WASM only for bounded runtime-installed behavior

## Read Next

- [Storefront Structure](./storefront-structure.md)
- [Catalog And Merchandising](./catalog-and-merchandising.md)
- [Checkout And Operations](./checkout-and-operations.md)
- [Linked Rust Backend](./linked-rust-backend.md)
- [WASM Extensions](./wasm-extensions.md)
