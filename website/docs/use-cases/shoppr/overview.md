---
title: Shoppr Overview
---

Shoppr is the reference Davenda customer app for ecommerce. It is useful because it does not stop
at a catalog grid or a checkout page. The app shows how one customer project owns the storefront,
theme, routes, admin surfaces, linked Rust backend, and runtime-installed WASM without becoming a
fork of the platform.

## Start With The Customer App Boundary

The app boundary lives in `apps/shoppr/` and is easiest to understand by reading these files in
order:

1. `apps/shoppr/app.toml`
   - declares the customer app identity
   - installs official modules such as `cms`, `commerce`, `memberships`, `events`, `admin`, and
     `ops`
   - pins the active theme and installed WASM package
   - declares the three Shoppr sites and their supported locales
2. `apps/shoppr/platform.dev.toml`
   - supplies the runtime settings for local HTTP, sessions, Redis, Postgres, object storage,
     jobs, and asset publication
   - shows that the customer app owns site config and localized-route policy in config as well as
     in the manifest
3. `apps/shoppr/crates/shoppr-app/src/lib.rs`
   - is the customer composition root
   - loads the manifest and config
   - resolves official modules
   - loads installed extensions
   - injects the linked customer backend plugin
   - builds the customer-root runtime plan
4. `apps/shoppr/crates/shoppr-bin/src/main.rs`
   - turns the customer app into an actual binary with `describe`, `validate`, `migrate`,
     `assets`, `serve`, `up`, and linked-backend inspection commands

That sequence is the main lesson: Shoppr is not just a template folder. It is a full customer app
that owns its own lifecycle.

## What Shoppr Teaches About Ecommerce

Shoppr is a good teaching app because it keeps the important ecommerce concerns close together:

- `apps/shoppr/templates/pages/home.html`
  - the public home page is not just editorial copy; it routes the customer toward catalog,
    collections, cart, account, admin, and dev tools
  - it also exposes the multi-site story in the page itself
- `apps/shoppr/templates/commerce/catalog.html`
  - shows the main browse loop for the storefront
  - collection-first browsing and product-detail handoff are explicit in the markup
- `apps/shoppr/templates/commerce/product-detail.html`
  - shows product detail as the decision point before cart and checkout
  - includes interactive gallery, accordion, and size-picker hooks layered on top of HTML-first
    markup
- `apps/shoppr/templates/commerce/cart.html`,
  `apps/shoppr/templates/commerce/checkout.html`, and
  `apps/shoppr/templates/commerce/checkout-confirmation.html`
  - show the public commerce journey all the way into checkout and confirmation
- `apps/shoppr/templates/account/` and `apps/shoppr/templates/memberships/`
  - show how customer continuity continues after checkout
- `apps/shoppr/templates/admin/`, `apps/shoppr/templates/cms/`, and
  `apps/shoppr/templates/commerce/orders.html`
  - show that a believable store needs operator surfaces alongside the public storefront

Shoppr therefore teaches ecommerce through the app itself rather than through a separate tutorial
domain model.

## Sites, Locales, And Shared Ownership

Shoppr is also the main use-case guide for Davenda's site model.

`apps/shoppr/app.toml` defines three sites:

- `shoppr-uk`
- `shoppr-fr`
- `shoppr-pl`

Each site declares:

- a canonical domain
- additional domains
- a default locale
- the locales that remain valid on that site

`apps/shoppr/platform.dev.toml` repeats the same shape for runtime config. This is important
because it shows the difference between:

- app manifest ownership
  - what the customer app claims to be
- runtime config ownership
  - how the active deployment resolves those sites locally

On the frontend, the multi-site story is not hidden:

- `apps/shoppr/templates/pages/home.html`
  renders the three-market cards
- `apps/shoppr/theme/assets/site.js`
  renders the market and locale switcher panels based on the current host and pathname

The point is not just that Davenda can route multiple sites. The point is that the customer app
owns the site policy directly.

## Theme And Frontend Ownership

Shoppr's theme is customer-owned and checked in under:

- `apps/shoppr/theme/assets/site.css`
- `apps/shoppr/theme/assets/site.js`
- `apps/shoppr/theme/tokens.toml`

Those files show three useful patterns:

- the customer app owns its visual system rather than inheriting a platform skin
- the theme can stay HTML-first while adding richer behavior through a small JS layer
- published assets still flow through Davenda's asset pipeline because templates use
  `asset('theme/assets/...')`

Read `apps/shoppr/theme/assets/site.js` when you want to see how the app layers interaction onto
plain templates. It currently drives:

- market and locale switcher panels
- the home-page campaign carousel
- PDP accordions
- PDP size selection
- PDP gallery thumbnails

That file is especially useful if you want to understand how Davenda customer apps can stay
server-rendered while still feeling like modern retail frontends.

## Linked Rust And WASM In One Store

Shoppr deliberately shows both customization paths.

The first-party path is linked Rust:

- `apps/shoppr/crates/shoppr-backend/src/lib.rs`
  - registers the linked customer plugin
  - exposes checkout and verified-webhook hooks
  - points back to customer-owned backend docs
- `apps/shoppr/backend/shoppr-loyalty-backend/`
  - contains the shared domain logic that the linked plugin wraps

The bounded runtime-installed path is WASM:

- `apps/shoppr/extensions/README.md`
  - explains why Shoppr keeps first-party logic out of WASM
- `apps/shoppr/extensions/shoppr-waitlist-tools/package.toml`
  - declares the installed package, its handler, and its target
- `apps/shoppr/crates/shoppr-app/src/extensions.rs`
  - loads the pinned package and compiles the checked-in WAT into a runtime artifact during
    bootstrap

Seeing both in one app matters. Shoppr teaches that:

- linked Rust is the primary path for customer-owned commerce policy
- WASM is the bounded path for replaceable runtime-installed behavior

## Where To Go Next

Use the other Shoppr guides for deeper slices:

- `catalog-and-merchandising`
  - storefront structure, custom pages, product routes, collections, sites, locales, and theme
    files
- `checkout-and-operations`
  - checkout flow, linked backend hooks, WASM, admin pages, order operations, and ops-facing
    runtime touchpoints
