---
title: Catalog And Merchandising
---

This guide uses Shoppr to explain how Davenda wants a catalog-driven store to be built through a
customer app, not assembled from a fixed storefront widget set.

## The Catalog Files To Read First

Start with these files:

- `apps/shoppr/catalog.toml`
- `apps/shoppr/templates/pages/home.html`
- `apps/shoppr/templates/commerce/catalog.html`
- `apps/shoppr/templates/commerce/collection-grid.html`
- `apps/shoppr/templates/commerce/product-grid.html`
- `apps/shoppr/templates/commerce/collection-detail.html`
- `apps/shoppr/templates/commerce/product-detail.html`

Read that list in order. It takes you from source data into real storefront presentation.

## How Shoppr Treats Merchandising

Shoppr does not treat merchandising as “whatever the catalog module gives us by default.”

Instead:

- the catalog module owns the reusable route and order model
- the customer app owns the visual browse loop
- fragments such as `collection-grid.html` and `product-grid.html` let the same data appear in
  several contexts without duplicating page markup

That split is one of the most useful patterns in the repo.

## Home Page As A Merchandising Surface

`apps/shoppr/templates/pages/home.html` is not only a landing page. It is the top of the store's
merchandising funnel.

It does all of these at once:

- introduces campaign framing
- advertises the active market and locale context
- links into collections and products
- reuses catalog fragments
- keeps developer-facing and operator-facing routes visible for the demo

If you are adapting Shoppr, keep the structure but replace:

- campaign copy
- featured collections
- site-specific links
- category framing

## Catalog Listing And Collection Pages

The broad browse surfaces are:

- `apps/shoppr/templates/commerce/catalog.html`
- `apps/shoppr/templates/commerce/collections.html`
- `apps/shoppr/templates/commerce/collection-detail.html`

Use them to study three different jobs:

- broad browse
- collection inventory view
- collection-specific product decision flow

That separation is useful because many real stores need all three.

## Product Cards And Collection Cards

The reusable display pieces are:

- `apps/shoppr/templates/commerce/product-grid.html`
- `apps/shoppr/templates/commerce/collection-grid.html`

These files matter because they show the cleanest way to reuse merchandising markup across:

- the home page
- the main catalog
- collection landing
- related-product sections on PDPs

This is the point where many apps drift into duplicated template code. Shoppr shows the cleaner
path.

## Product Detail As A Merchandising Decision Surface

`apps/shoppr/templates/commerce/product-detail.html` is where browse turns into intent.

It combines:

- product framing
- gallery and gallery controls
- size and quantity
- add-to-cart and buy-now forms
- supporting detail sections
- related product continuation

That page is also where theme JS and CSS prove their value. The structure stays server-rendered,
but the interactions feel like retail.

## Catalog Admin As The Operator Side Of Merchandising

Merchandising is not only public presentation. Shoppr also ships:

- `apps/shoppr/templates/commerce/catalog-admin.html`

That page demonstrates the day-one operator boundary for:

- product copy review
- collection placement
- visibility controls

It is intentionally not a full product information management suite. That honesty is part of the
demo's value.

## What To Adapt For Your Own Store

Copy these ideas first:

- route separation by customer decision stage
- reusable product and collection fragments
- customer-owned home page that still consumes catalog data
- a bounded operator surface for live catalog management

Then adapt:

- category taxonomy
- product-card density
- merchandising voice
- PDP interaction design

## Read Next

- [Storefront Structure](./storefront-structure.md)
- [Custom Pages And CMS](./custom-pages-and-cms.md)
- [Commerce Module Reference](../../reference/modules/commerce.md)
