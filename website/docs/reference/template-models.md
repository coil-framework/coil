---
title: Template Models
---

Davenda templates render against typed models, not an unstructured bag of JSON.

That is why the theme docs can talk about `product`, `cart`, `page`, or `site` as real concepts.
Those names are not arbitrary. They are the public rendering surface exposed by core and the
installed modules for the current route.

## What This Page Covers

Use this page when you want to know:

- which built-in template models exist today
- what kind of data they expose
- where those models come from
- how to decide whether a value belongs in a built-in model, linked Rust hook, CMS record, or
  extension

For template syntax, read [Template Language](./template-language.md). For theme file structure,
read [Theme Structure](./theme-structure.md).

## How Model Binding Works

At render time, Davenda resolves:

1. the request host, site, and locale
2. the route and page handler
3. the module-owned or customer-owned data for that route
4. a typed render model
5. the template that consumes that model

That means a template only sees the models that are valid for the current route.

The consequence is important:

- templates stay predictable
- module contracts remain typed
- customer code extends behaviour through supported hooks and repositories instead of inventing
  random top-level model names in HTML

## Core Models Available Across Most Pages

These models are part of the common render surface for most browser routes.

### `site`

The current site or market being served.

Typical values include:

- site id
- label
- host and domain mapping
- default locale
- enabled locales
- brand-facing metadata used in headers, footers, and SEO

Use it for:

- site switchers
- footer metadata
- market-aware navigation
- canonical host logic in head markup

### `locale`

The resolved locale for the current request.

Typical values include:

- locale code
- language tag
- route prefix behaviour
- fallback relationship to the site default

Use it for:

- language switchers
- translated navigation
- locale-aware alternate links

### `route`

The resolved page route.

Typical values include:

- public path
- route kind
- request parameters
- page-level metadata assembled by the runtime

Use it for:

- active navigation states
- breadcrumbs
- diagnostics while developing

### `seo`

The resolved SEO metadata for the current page.

Typical values include:

- title
- description
- canonical URL
- alternate locale links
- robots directives
- Open Graph fields
- JSON-LD blocks prepared by the runtime

Use it in `<head>` layouts and structured data fragments. For the full SEO control surface, read
[SEO](./seo.md).

### `viewer`

The current browser principal as seen by the runtime.

Typical values include:

- anonymous vs signed-in state
- account summary fields for account pages
- session-scoped presentation state

Use it for:

- account nav
- member-only UI branches
- sign-in and sign-out affordances

## Commerce Models

When the commerce module owns the route, templates can receive commerce-specific models.

### `product`

Used on product detail pages.

Typical fields include:

- id or handle
- SKU
- title
- summary
- price and currency
- imagery
- badges
- option summaries
- product kind
- collection membership
- add-to-cart form inputs

See Shoppr for a live example:

- `apps/shoppr/templates/shoppr/product.html`
- `apps/shoppr/templates/components/product-card.html`

### `collection`

Used on collection and catalogue listing pages.

Typical fields include:

- handle
- title
- summary
- visible products
- merchandising labels
- site-specific availability

### `cart`

Used on cart routes.

Typical fields include:

- line items
- quantity controls
- totals
- applied notes or metadata
- checkout actions

The cart model is where HTML-first interactions matter most. Davenda renders a complete form and
then layers JavaScript on top for richer behaviour. See:

- [Storefront Structure](../use-cases/shoppr/storefront-structure.md)
- [Request And Render Lifecycle](../core-concepts/request-and-render-lifecycle.md)

### `checkout`

Used on checkout pages.

Typical fields include:

- order draft summary
- contact and shipping fields
- payment handoff state
- validation errors
- customer-hook decisions surfaced back into the form

## CMS Models

When the CMS module owns the route, templates can receive content-centric models.

### `page`

Used for CMS-backed content pages.

Typical fields include:

- title
- slug
- summary
- body HTML
- publication status
- live path

Use it for:

- marketing pages
- landing pages
- help and policy pages

See Shoppr:

- `apps/shoppr/templates/shoppr/page.html`
- `apps/shoppr/templates/pages/home.html`

### `navigation`

Used for shared navigation fragments.

Typical fields include:

- label
- href
- grouping
- visibility rules contributed by the current site or locale

## Membership And Event Models

Installed modules extend the render surface with their own typed data.

Examples include:

- membership plans and entitlements
- event listings and booking availability
- account history and membership state

Those shapes are intentionally contributed by the module layer rather than redefined inside the
theme.

## Admin Models

Admin routes expose admin-specific models for:

- queue and job state
- CMS inventory
- catalogue and order inspection
- audit and operator surfaces

Admin templates should treat these as typed operator-facing models, not as a copy of the public
storefront models.

## Can Customer Apps Add Their Own Models?

Today the supported public model is:

- core contributes common models
- official modules contribute module models
- customer apps shape the values that flow through those models by using linked Rust hooks,
  repositories, CMS content, and extensions

Customer apps should not treat the template language as an arbitrary view-model injection system.

If you need new behaviour, start with one of these questions:

1. Is this new data really content? Put it in CMS or repository records.
2. Is this a policy decision? Put it in linked Rust hooks.
3. Is this a bounded runtime-installed enhancement? Use a WASM extension.
4. Is this reusable domain functionality? It may belong in an official module instead.

## How To Discover The Right Model In Practice

Use this workflow:

1. Find the route in Shoppr or Gitly.
2. Open the page template and its surrounding layout/fragments.
3. Read the relevant use-case page to understand the route contract.
4. Read the module reference page if the route is module-owned.
5. Only then change the template.

That sequence avoids a common mistake: changing HTML without understanding which typed model owns
the route.

## Common Mistakes

- Assuming every page gets every model. Davenda binds models per route.
- Treating `page` and `product` as generic names you can reuse for unrelated shapes.
- Trying to invent arbitrary top-level template models instead of using supported hooks and
  repositories.
- Putting business logic in the template instead of shaping the model earlier in the request flow.

## Read Next

- [Template Language](./template-language.md)
- [Theme Structure](./theme-structure.md)
- [Storefront Structure](../use-cases/shoppr/storefront-structure.md)
- [Product Structure](../use-cases/gitly/product-structure.md)
