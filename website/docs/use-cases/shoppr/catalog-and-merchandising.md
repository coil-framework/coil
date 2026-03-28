---
title: Catalog And Merchandising
---

This guide uses Shoppr to explain how Davenda wants a storefront to be structured. The important
idea is that the browse loop is made out of customer-owned routes and templates, not out of a
single platform-owned catalog widget.

## The Storefront Route Structure

Shoppr splits the public storefront into distinct route families:

- `apps/shoppr/templates/pages/home.html`
  - the campaign-led storefront landing page
  - acts as the top of the merchandising funnel
- `apps/shoppr/templates/commerce/catalog.html`
  - the broad browse surface
  - explains the role of collections versus product-detail routes directly in the page copy
- `apps/shoppr/templates/commerce/collection-detail.html`
  - narrows the customer from a broad catalog into a merchandising context
- `apps/shoppr/templates/commerce/product-detail.html`
  - is the point where the customer chooses quantity, size, and next action
- `apps/shoppr/templates/commerce/cart.html`
  - continues the same public flow without forcing a context switch into account or admin

That route structure is one of the clearest lessons in Shoppr: the customer app can make the
commerce journey obvious in its own templates.

## Home Page As A Merchandising Surface

`apps/shoppr/templates/pages/home.html` is worth reading carefully because it does several jobs at
once:

- it owns the home-page campaign layout and call-to-action hierarchy
- it surfaces the multi-site story directly in the storefront
- it links cleanly to catalog, collections, cart, account, admin, and dev tools
- it reuses `commerce/collection-grid` and `commerce/product-grid` fragments rather than copying
  catalog markup into the page

This is a good example of Davenda's intended customer-app boundary: the app keeps ownership of the
front page while reusing smaller fragments where that helps.

## Product And Collection Presentation

The product and collection list fragments live in:

- `apps/shoppr/templates/commerce/product-grid.html`
- `apps/shoppr/templates/commerce/collection-grid.html`

Those fragments are useful because they show how the same catalog data can appear:

- on the home page
- in the main catalog
- inside collection detail
- alongside product detail as related browsing

That is the practical modularity lesson. The platform does not force one fixed storefront page
type. The customer app composes its own fragments.

## Product Detail As The Decision Point

`apps/shoppr/templates/commerce/product-detail.html` shows the most important commerce page in the
demo.

It keeps these concerns on one screen:

- collection breadcrumbs
- product image and gallery state
- price and summary
- size interaction
- quantity input
- add-to-cart and buy-now forms
- route back to collection and cart
- related product browsing below the main PDP

The interaction layer is not hidden in a framework black box:

- `apps/shoppr/theme/assets/site.js`
  - drives the gallery thumbnails
  - toggles the PDP accordions
  - manages the size-picker state
- `apps/shoppr/theme/assets/site.css`
  - gives those controls their visual states and layout

That combination is useful to study because it shows how a customer app can stay server-rendered
while still owning premium retail behavior.

## Sites And Locales

Shoppr is also the best example of the site-and-locale model in a commerce context.

The customer app manifest in `apps/shoppr/app.toml` declares:

- one app id: `shoppr`
- three sites: UK, France, and Poland
- localized routes enabled
- supported locales per site

The runtime config in `apps/shoppr/platform.dev.toml` mirrors the same sites under `[[sites]]`.

That matters because it lets the customer app express:

- host-based storefront resolution
- market-specific branding through `brand_name`
- different default locales
- different allowed locale sets on different sites

On the frontend, `apps/shoppr/theme/assets/site.js` turns those site and locale choices into
market and language switcher panels. On the storefront, `apps/shoppr/templates/pages/home.html`
turns the same concept into visible market cards.

The key lesson is simple:

- use sites when assortment, host, or market framing changes
- use locales when language and route localization change inside that site

## Themes As Part Of Merchandising

Shoppr's visual language is not incidental. The theme files are part of the merchandising story:

- `apps/shoppr/theme/tokens.toml`
  - carries token-level theme data
- `apps/shoppr/theme/assets/site.css`
  - defines the store's layout, motion, focus styling, card treatment, and visual hierarchy
- `apps/shoppr/theme/assets/site.js`
  - layers on the homepage carousel and PDP interactions

The theme is therefore not just decorative. It helps make the browse loop credible.

## Custom Pages And CMS Surfaces

Catalog pages are not the only thing Shoppr uses to merchandise.

Read:

- `apps/shoppr/templates/cms/pages.html`
- `apps/shoppr/templates/cms/page.html`
- `apps/shoppr/templates/cms/preview.html`

These pages show that the same customer app can also own:

- editorial page inventory
- draft and publish flows
- preview routes
- redirect and navigation management

That matters because real commerce sites mix product routes with editorial routes. Shoppr teaches
that both live in the same customer app boundary.

## Practical Reading Order

If you want to learn storefront structure from Shoppr, read these files in order:

1. `apps/shoppr/app.toml`
2. `apps/shoppr/templates/pages/home.html`
3. `apps/shoppr/templates/commerce/catalog.html`
4. `apps/shoppr/templates/commerce/collection-detail.html`
5. `apps/shoppr/templates/commerce/product-detail.html`
6. `apps/shoppr/theme/assets/site.css`
7. `apps/shoppr/theme/assets/site.js`

That sequence shows how Davenda expects a customer app to turn catalog data, localized routes, and
theme assets into an actual storefront.
