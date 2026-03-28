---
title: Custom Pages And CMS
---

Shoppr is a store, but it is also a good CMS example. This guide shows how custom pages,
navigation, preview, and redirects fit into the same customer app as the storefront.

## Why This Matters In A Store

Real commerce apps need more than product routes. They also need:

- landing pages
- editorial pages
- policies and help content
- campaign redirects
- navigation changes tied to launches

Davenda keeps those inside the same customer app boundary instead of pushing them into a separate
tooling story.

## The Main CMS Files

Read these files together:

- `apps/shoppr/templates/cms/page.html`
- `apps/shoppr/templates/cms/pages.html`
- `apps/shoppr/templates/cms/preview.html`
- `apps/shoppr/templates/cms/navigation.html`
- `apps/shoppr/templates/cms/redirects.html`
- `apps/shoppr/content/page-types/home.toml`
- `apps/shoppr/content/page-types/landing_page.toml`

Those files show both sides of the CMS surface:

- public rendering
- operator inventory and publication workflow

## Where The CMS Routes Come From

The runtime surface is declared by the CMS module in
`crates/davenda-cms/src/module/platform/manifest.rs`.

That manifest adds:

- `/pages/{slug}`
- `/admin/pages`
- `/admin/pages/preview`
- `/admin/navigation`
- `/admin/redirects`

Shoppr then supplies the templates that make those routes product-specific.

## How Preview Fits The Publishing Story

`apps/shoppr/templates/cms/preview.html` is worth reading because it shows that preview is part of
publication, not a sidecar concern.

That matters in practice because preview is connected to:

- draft state
- page save and publish actions
- navigation edits
- redirect changes
- SEO and canonical continuity

## Navigation And Redirects Are Product Work

Two files make this especially clear:

- `apps/shoppr/templates/cms/navigation.html`
- `apps/shoppr/templates/cms/redirects.html`

These are useful because they demonstrate a practical Davenda idea:

- navigation is part of product composition
- redirects are part of launch and editorial operations

Neither should be treated as “someone else's infrastructure problem.”

## How To Add A Custom Page In This Model

In a Davenda customer app, adding a custom page usually means:

1. decide the page type and route role
2. add or update the page type under `content/page-types/`
3. add or adapt the template under `templates/cms/` or `templates/pages/`
4. make sure navigation and redirects reflect the new route

Shoppr gives you a concrete reference app for that flow.

## Adapt This For Your App

Copy these parts:

- keep editorial routes in the same app as storefront routes
- use preview and publish as real workflow concepts
- manage navigation and redirects in the same operator boundary

Then adapt:

- page-type vocabulary
- editorial review steps
- campaign-specific landing page templates

## Read Next

- [Catalog And Merchandising](./catalog-and-merchandising.md)
- [CMS Module Reference](../../reference/modules/cms.md)
- [SEO Reference](../../reference/seo.md)
