---
title: Checkout And Operations
---

Shoppr is not only a storefront guide. It is also the main example of how Davenda expects a
customer app to connect checkout, account continuity, admin surfaces, and operator workflows.

## Checkout Through The Customer App

The public checkout path is visible in the template tree:

- `apps/shoppr/templates/commerce/cart.html`
  - basket review and quantity updates
- `apps/shoppr/templates/commerce/checkout.html`
  - the checkout form and payment handoff page
- `apps/shoppr/templates/commerce/checkout-confirmation.html`
  - post-checkout confirmation

These templates matter because they are not isolated widgets. They sit inside the same customer app
that owns:

- the browse loop
- the account area
- the admin order queue
- the theme and frontend interaction layer

That is the main checkout lesson in Shoppr: the customer app owns the whole journey.

## Account And Membership Continuity

After checkout, Shoppr continues the same story in:

- `apps/shoppr/templates/pages/account.html`
- `apps/shoppr/templates/account/dashboard.html`
- `apps/shoppr/templates/account/orders.html`
- `apps/shoppr/templates/account/summary-panels.html`
- `apps/shoppr/templates/memberships/account.html`

Those files are useful because they show how the app keeps order and membership state visible
without pretending the platform has become a generic dashboard generator. The templates explicitly
explain:

- current browser-session continuity
- latest order state
- pending-payment messaging after provider return
- membership activation as a post-checkout outcome

If you want to see how account surfaces relate back to commerce, these are the files to read.

## Linked Rust Backend For First-Party Store Logic

Shoppr uses linked Rust for first-party customer behavior.

The key files are:

- `apps/shoppr/crates/shoppr-backend/src/lib.rs`
  - exposes the linked plugin
  - registers checkout and verified-webhook hooks
  - publishes a stable linked-plugin summary
- `apps/shoppr/backend/shoppr-loyalty-backend/src/lib.rs`
  - contains the customer-owned domain logic for order review, loyalty preview, and CRM routing
- `apps/shoppr/crates/shoppr-bin/src/main.rs`
  - exposes commands such as `linked-backend describe` and `linked-backend demo`

The point of these files is not just that Shoppr has custom business logic. The point is that the
customer app owns that logic through the customer-root workspace, not through an ad hoc sidecar or
patch to platform code.

## Runtime-Installed WASM For Bounded Extensions

Shoppr also demonstrates the bounded extension path.

Read:

- `apps/shoppr/extensions/README.md`
- `apps/shoppr/extensions/shoppr-waitlist-tools/package.toml`
- `apps/shoppr/extensions/shoppr-waitlist-tools/README.md`
- `apps/shoppr/crates/shoppr-app/src/extensions.rs`

Together, these files show:

- the extension is pinned in `app.toml`
- it is runtime-installed rather than linked into the customer binary
- the customer app compiles the checked-in WAT source into the runtime artifact path during
  bootstrap
- the current demo uses a render-hook target rather than a full transactional ownership boundary

That makes Shoppr a good guide to the difference between:

- linked Rust for first-party store logic
- WASM for bounded runtime-installed behavior

## Admin And Support Surfaces

The back-office path is visible in the checked-in templates:

- `apps/shoppr/templates/admin/dashboard.html`
  - the main operator control room
- `apps/shoppr/templates/commerce/orders.html`
  - store-wide order queue
- `apps/shoppr/templates/commerce/order-detail.html`
  - per-order support surface
- `apps/shoppr/templates/commerce/catalog-admin.html`
  - live catalog copy management
- `apps/shoppr/templates/admin/audit.html`
  - audit visibility
- `apps/shoppr/templates/cms/pages.html`
  - CMS page draft and publish workflow

These are important because they prove Shoppr is teaching more than storefront pages. It teaches
what a usable small-store operator surface looks like when it lives in the same customer app.

## Runtime And Operational Touchpoints

Shoppr's runtime and lifecycle ownership are visible in a few specific files:

- `apps/shoppr/platform.dev.toml`
  - local database, Redis, object storage, jobs, observability, session, and Stripe config
- `apps/shoppr/crates/shoppr-app/src/lib.rs`
  - loads the manifest and config, resolves official modules, loads extensions, injects linked
    plugins, and builds the runtime plan
- `apps/shoppr/crates/shoppr-bin/src/main.rs`
  - exposes `describe`, `validate`, `migrate apply`, `assets publish`, `serve`, and `up`
- `apps/shoppr/README.md`
  - explains the first-run path and customer-owned lifecycle commands

This is where Shoppr becomes more than a theme demo. It shows that the customer project owns:

- validation
- migration application
- asset publication
- runtime startup

## Suggested Reading Order

If your goal is to understand checkout plus operations through Shoppr, read these files in order:

1. `apps/shoppr/templates/commerce/cart.html`
2. `apps/shoppr/templates/commerce/checkout.html`
3. `apps/shoppr/templates/commerce/checkout-confirmation.html`
4. `apps/shoppr/templates/account/orders.html`
5. `apps/shoppr/templates/memberships/account.html`
6. `apps/shoppr/crates/shoppr-backend/src/lib.rs`
7. `apps/shoppr/extensions/shoppr-waitlist-tools/package.toml`
8. `apps/shoppr/templates/commerce/orders.html`
9. `apps/shoppr/templates/admin/dashboard.html`
10. `apps/shoppr/platform.dev.toml`
11. `apps/shoppr/crates/shoppr-bin/src/main.rs`

That order shows how Davenda expects one customer app to carry public commerce, customer
continuity, bounded extensions, and operator workflows together.
