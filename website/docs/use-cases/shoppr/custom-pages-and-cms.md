---
title: Custom Pages And CMS
---

A serious ecommerce app needs more than product routes. Shoppr uses Davenda’s CMS surfaces to show how custom pages, navigation, preview, and redirects fit into the same customer app.

## Why This Matters

Real stores mix:

- product and collection routes
- campaign landing pages
- policy pages
- editorial content
- promotional redirects

If the framework treats those as separate worlds, the product becomes awkward to operate.

## The Main CMS Templates

Shoppr’s CMS-facing templates include:

- `templates/cms/page.html`
- `templates/cms/pages.html`
- `templates/cms/preview.html`
- `templates/cms/navigation.html`
- `templates/cms/redirects.html`

These files represent:

- the public page render
- the page inventory for operators
- preview behavior
- navigation management
- redirect management

## Adding A Custom Page

At the customer-app level, a custom page usually involves:

1. deciding the route and content purpose
2. creating or adapting a template
3. wiring the page through CMS data or route configuration
4. making sure navigation and SEO metadata reflect the new page

Shoppr shows that this does not require leaving the same application boundary used by storefront and account routes.

## Why Preview And Redirects Are In The Same App

Preview and redirect handling belong beside the pages because they affect publication behavior and public URLs directly.

That keeps:

- editorial review
- route changes
- SEO continuity
- legacy URL handling

inside one product model instead of scattering them into separate tools.

## What To Read Next

- [Storefront Structure](./storefront-structure.md)
- [SEO Reference](../../reference/seo.md)
- [Shoppr Checkout And Operations](./checkout-and-operations.md)
