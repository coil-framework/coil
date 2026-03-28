---
title: Storefront Structure
---

This guide uses Shoppr to show how Davenda expects a storefront to be structured in a real customer
app.

The key idea is simple: the storefront is a set of customer-owned pages and templates that use
module data, not one platform-owned catalog widget.

## Start With The Route Shape

The right mental model is a decision journey, not a bag of pages:

```text
home -> catalog -> collection -> product -> cart
```

Each step has a different job:

- home
  - campaign entry and brand framing
- catalog
  - broad browse and discovery
- collection
  - merchandising context
- product
  - buying decision
- cart
  - continuity into checkout

Davenda works best when the storefront is explicit at this level instead of hiding the whole browse
loop behind one generic listing view.

## Where The Routes Come From

The route contracts come from the commerce module manifest in
`crates/davenda-commerce/src/module/platform/manifest.rs`.

That manifest defines routes such as:

- `/shop`
- `/shop/collections`
- `/shop/collections/{collection_slug}`
- `/shop/products/{product_slug}`
- `/cart`

Shoppr then provides the customer-owned templates that make those routes feel like Shoppr instead
of a generic store.

## Layouts And Shared Shell

The storefront shell should separate document chrome from route-specific markup.

A good pattern looks like this:

```html
<html xmlns:dv="https://davenda.dev" dv:attr="lang=${locale}">
  <body>
    <dv:include src="navigation/primary.html" />
    <main>
      <dv:slot />
    </main>
  </body>
</html>
```

Then individual route templates fill the slot.

That keeps:

- document shell in one place
- navigation reusable
- promotional fragments reusable
- route templates focused on route work

## Home Page Structure

`apps/shoppr/templates/pages/home.html` is worth studying because it combines several concerns that
real stores need:

- campaign framing
- links into catalog and collections
- market and locale visibility
- links to cart, account, admin, and CMS surfaces
- reused catalog fragments instead of duplicated markup

If you are designing your own storefront, treat the home page as a product entry surface, not just
a marketing hero.

## Product Detail As The Critical Page

`apps/shoppr/templates/commerce/product-detail.html` is the most important page in the demo.

It keeps together:

- breadcrumbs and collection context
- media gallery markup
- size and quantity controls
- add-to-cart and buy-now actions
- supporting product facts and details
- related product browsing

This is a good Davenda page to copy structurally because it stays HTML-first even when the theme
adds richer behavior.

## How The Product Template Gets Its Model

This is the part that often feels disconnected if you only read the template.

In Shoppr, the product-detail page is not a free-floating HTML file. The binding path is:

1. the commerce module contributes route `commerce.product-detail`
2. that route is wired to template `commerce/product-detail`
3. the runtime adds the shared request keys and then appends `product`, `productCards`,
   `hasProduct`, and related fields for that route
4. `apps/shoppr/templates/commerce/product-detail.html` consumes those fields directly

The important consequence is that this markup:

```html
<h1 dv:text="${product.name}">Harbor Cap</h1>
<p class="product-page__price" dv:text="${product.price}">GBP 29</p>
<p dv:text="${product.summary}">Product summary</p>
```

only works because the runtime has already shaped a `product` object for `commerce.product-detail`.

That object is not being invented inside the template. It is supplied by the runtime render-model
binding for that route.

## Progressive Enhancement Layer

The interaction layer lives in:

- `apps/shoppr/theme/assets/site.js`
- `apps/shoppr/theme/assets/site.css`

On the storefront, that layer currently owns:

- home carousel behavior
- market and locale switcher panels
- PDP gallery thumbnails
- PDP accordions
- size selection state
- visible focus styling and reduced-motion behavior

That is the pattern to copy:

- make the markup correct first
- add JavaScript as a progressive layer
- keep the route and form structure server-rendered

## Full Implementation

If you want the complete Shoppr storefront after learning the structure:

- `apps/shoppr/templates/pages/home.html`
- `apps/shoppr/templates/commerce/catalog.html`
- `apps/shoppr/templates/commerce/collection-detail.html`
- `apps/shoppr/templates/commerce/product-detail.html`
- `apps/shoppr/templates/commerce/cart.html`
- `apps/shoppr/templates/layouts/base.html`
- `apps/shoppr/templates/layouts/storefront.html`
- `apps/shoppr/templates/navigation/primary.html`
- `apps/shoppr/templates/components/hero.html`
- `apps/shoppr/theme/assets/site.js`

The route and model side of the same page lives here:

- `crates/davenda-commerce/src/module/platform/manifest.rs`
- `crates/davenda-runtime/src/render/model.rs`

## What To Copy Into Your Own App

If you are building a Davenda storefront, copy this structure before you copy Shoppr's exact look:

1. top-level page per customer decision stage
2. layouts and fragments for repeated shell markup
3. HTML-first forms for cart and checkout flow
4. small theme JS for enrichment, not ownership of the product model

## Common Mistakes

- Putting all storefront logic into one oversized home or catalog page.
- Treating the customer app as a template override layer instead of the real storefront owner.
- Letting JavaScript own critical product behavior that the server-rendered page should still
  express.

## Read Next

- [Catalog And Merchandising](./catalog-and-merchandising.md)
- [Sites, Locales, And Theme Variants](./sites-locales-and-theme-variants.md)
- [Template Language Reference](../../reference/template-language.md)
