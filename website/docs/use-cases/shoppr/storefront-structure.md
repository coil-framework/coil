---
title: Storefront Structure
---

Shoppr is the main example of how Davenda expects a storefront to be assembled from customer-owned pages, module-provided data, and progressive enhancement layered on top of HTML.

## The Core Storefront Pages

The public browse loop is intentionally split across distinct templates:

- `templates/pages/home.html`
- `templates/commerce/catalog.html`
- `templates/commerce/collection-detail.html`
- `templates/commerce/product-detail.html`
- `templates/commerce/cart.html`

That split matters because it keeps the customer journey explicit:

- home establishes editorial direction
- catalog establishes broad browse
- collection detail narrows the merchandising context
- product detail is the decision point
- cart preserves continuity toward checkout

## Why The Home Page Matters

Shoppr’s home page is not just a marketing banner. It does four jobs:

- introduces the current campaign and product emphasis
- exposes the site and locale story visibly
- links into the browse loop and developer-facing routes
- reuses smaller fragments instead of duplicating catalog markup directly

That is how Davenda expects a customer app to own the top-level product shell while still reusing fragmentized module data.

## Layouts And Navigation

The storefront shell is built from:

- `templates/layouts/base.html`
- `templates/layouts/storefront.html`
- `templates/navigation/primary.html`
- `templates/components/hero.html`

These files matter because they show where global structure lives:

- document shell
- navigation
- repeated hero or promotional structures
- footer and developer-facing links

The customer app keeps ownership of that shell instead of treating it as a module default.

## Product Detail And Interactivity

The product-detail page is the best single place to study Davenda’s “interactivity layered on” model.

The HTML page still owns:

- gallery structure
- size selection controls
- quantity and add-to-cart forms
- product facts
- expandable details

Then `theme/assets/site.js` enhances those controls with:

- gallery switching
- accordion behavior
- variant-like state for size selection

This is the pattern to copy in a serious storefront: keep the page correct without the script, then make it faster and richer when the script is present.

## What To Read Next

- [Catalog And Merchandising](./catalog-and-merchandising.md)
- [Custom Pages And CMS](./custom-pages-and-cms.md)
- [Template Language Reference](../../reference/template-language.md)
