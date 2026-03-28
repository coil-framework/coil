---
title: Commerce Module
---

The commerce module owns catalog, cart, checkout, order state, and the operator order queue.

Primary implementation files:

- `crates/coil-commerce/src/module/platform/manifest.rs`
- `apps/shoppr/templates/commerce/catalog.html`
- `apps/shoppr/templates/commerce/product-detail.html`
- `apps/shoppr/templates/commerce/cart.html`
- `apps/shoppr/templates/commerce/checkout.html`
- `apps/shoppr/templates/commerce/orders.html`

## Why It Exists

Commerce is not just a handful of storefront pages. It needs:

- catalog and collection data
- cart and checkout state
- order lifecycle
- payment-provider bridge points
- admin order operations
- event and membership integration

That reusable battery belongs in an official module.

## What It Provides

From `crates/coil-commerce/src/module/platform/manifest.rs`, commerce adds:

- migrations for catalog, checkout, and orders
- public routes for catalog, collections, product detail, cart, checkout, and confirmation
- account route `/account/orders`
- admin routes for `/admin/orders`, `/admin/orders/{order_id}`, and `/admin/catalog/products`
- domain-event jobs for order confirmation and refund follow-up
- search and report contributions
- a `commerce.payment-provider` webhook extension slot

## How To Enable It

```toml title="app.toml"
[modules]
enabled = ["commerce"]
```

```toml title="platform.dev.toml"
[modules]
enabled = ["commerce"]
```

Shoppr uses that exact pattern in `apps/shoppr/app.toml` and `apps/shoppr/platform.dev.toml`.

## How To Disable It

Remove `commerce` from the enabled module lists and then remove customer templates and links that
depend on commerce-owned routes such as `/shop`, `/cart`, `/checkout`, and `/admin/orders`.

## Config Expectations

Base commerce relies mostly on shared config:

- database
- jobs
- cache
- i18n
- SEO
- template loading

Payment-provider configuration is handled by add-on modules such as
[`commerce-payments-stripe`](./commerce-payments-stripe.md).

## Routes And Surfaces

Key public routes:

- `/shop`
- `/shop/collections`
- `/shop/collections/{collection_slug}`
- `/shop/products/{product_slug}`
- `/cart`
- `/checkout`
- `/checkout/confirmation`

Key account and admin routes:

- `/account/orders`
- `/admin/orders`
- `/admin/orders/{order_id}`
- `/admin/catalog/products`

## Required Auth Capabilities

Commerce requires:

- `catalog.product.read`
- `catalog.product.edit`
- `catalog.collection.edit`
- `checkout.session.create`
- `order.read`
- `order.refund.issue`

Those capability contracts are what let customer apps swap auth packages without patching the
module itself.

## How Customer Apps Extend It

Commerce exposes extension slots for:

- render hook: `commerce.pricing`
- webhook hook: `commerce.payment-provider`

Customer apps also extend commerce through:

- customer-owned templates under `apps/shoppr/templates/commerce/`
- linked checkout and verified-webhook hooks
- optional module bridges to memberships and events

Concrete example:

```html title="templates/commerce/product-detail.html"
<form method="post" action="/cart">
  <input type="hidden" name="sku" coil:attr="value=${product.sku}" />
  <button type="submit">Add to bag</button>
</form>
```

Commerce still owns cart, checkout, and order state. The customer app owns the product-detail page,
copy, merchandising layout, and progressive enhancement around that workflow.

The practical sequence is:

1. enable `commerce`
2. provide storefront templates under `templates/commerce/`
3. add linked checkout hooks for customer-owned order policy
4. add verified webhook hooks or payment-provider add-ons where needed

## Where To See It

Shoppr is the canonical example:

- public storefront in `apps/shoppr/templates/commerce/`
- account continuity in `apps/shoppr/templates/account/orders.html`
- operator views in `apps/shoppr/templates/commerce/orders.html`

## Common Mistakes

- Treating payment integration as part of base commerce instead of a separate provider module.
- Forgetting that account and admin order routes come from commerce too, not just the storefront.
- Implementing customer pricing logic in templates instead of linked hooks or bounded extensions.

## Read Next

- [Commerce Payments Stripe](./commerce-payments-stripe.md)
- [Memberships](./memberships.md)
- [Shoppr Checkout And Operations](../../use-cases/shoppr/checkout-and-operations.md)
