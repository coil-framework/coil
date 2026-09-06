---
title: Sites, Locales, And Theme Variants
---

Shoppr is the clearest current example of Coil's site model in a commerce app.

It shows one customer app serving several related storefronts with shared code, shared modules,
and controlled differences in host, locale, and branding.

## The Concrete Site Setup

Read these two files side by side:

- `apps/shoppr/app.toml`
- `apps/shoppr/platform.dev.toml`

Both define three sites:

- `shoppr-uk`
- `shoppr-fr`
- `shoppr-pl`

Each site declares:

- canonical domain
- additional domains or hosts
- display and brand names
- default locale
- supported locales

## Why Shoppr Uses Sites Instead Of Locale Alone

Shoppr uses sites because the differences are broader than language:

- different canonical hosts
- different brand framing
- different default locale
- potentially different assortment emphasis and campaign voice

Locale alone would not be a good place to model all of that.

## How Locale Still Matters Inside A Site

Even though Shoppr uses multiple sites, each site still supports multiple locales.

That gives the app two different levers:

- site for market boundary and host identity
- locale for language and route localisation inside that site

This is the model to copy when you want one shared product with multiple regional surfaces.

## Where The Site Differences Become Visible

Shoppr makes the site model visible in several layers:

- `apps/shoppr/templates/pages/home.html`
  - market cards and site-aware framing
- `apps/shoppr/theme/assets/site.js`
  - market and locale switcher panels
- `apps/shoppr/theme/assets/site.css`
  - the visual layer for those controls

That matters because the site model is not just runtime plumbing. The customer-facing UI should
make the current context obvious.

## Theme Variants Without Three Separate Apps

Shoppr does not create three different customer apps.

Instead it uses one app with:

- one shared template tree
- one shared theme asset set
- site-aware copy and branding
- shared module surfaces

That is the practical Coil pattern. Variation should come from explicit site and locale policy,
not from duplicating the whole app.

## Adapt This For Your Store

Add a new site when:

- host or brand changes
- market framing changes
- the canonical public identity is different

Add a locale when:

- the product is still the same site
- you mainly need language and route localisation changes

Shoppr is the concrete example to use when making that call.

## Read Next

- [Shoppr Overview](./overview/)
- [Sites, Locales, And Markets](../../core-concepts/sites-locales-and-markets/)
- [Internationalisation Reference](../../reference/internationalization/)
