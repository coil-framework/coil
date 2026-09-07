---
title: Sites, Locales, And Markets
---

Coil treats site and locale as runtime concerns. They are not just view-layer labels.

If you skip this distinction, multi-site behaviour becomes confusing quickly.

## What It Is

These three terms are related but different.

### Site

A site is a first-class delivery surface inside one customer application. A site usually has its own host bindings, default locale, brand identity, and assortment shape.

### Locale

Locale is the language and regional presentation context for a request. It affects routing, page rendering, metadata, and often product messaging.

### Market

Market is a commerce concept about selling conditions such as assortment, pricing, availability, or region-specific offers. It may line up with a site, but it is not the same concept.

## Why The Distinction Exists

Real products often need all three concerns, but not always in the same combination.

Examples:

- one application can serve multiple sites
- one site can support multiple locales
- one market strategy can span multiple sites, or one site can map to one market

If those distinctions are flattened together, route resolution and commerce behaviour become much harder to reason about.

## How It Works

In Coil, the request path first resolves the site from the host and then applies locale-aware route matching within that site context.

That lets the runtime carry site and locale through:

- route resolution
- canonical URL generation
- feature flag evaluation
- render-model assembly
- product visibility checks

Markets then layer on top where commerce rules need them.

## Shoppr As The Concrete Example

Shoppr is the current multi-site reference example. It defines three sites in:

- `apps/shoppr/app.toml`
- `apps/shoppr/platform.dev.toml`
- `apps/shoppr/platform.toml`

Those sites are:

- `shoppr-uk`
- `shoppr-fr`
- `shoppr-pl`

They share one customer app and one binary, but differ in:

- canonical host
- additional host bindings
- display name
- brand name
- default locale

That is exactly the kind of case where `[[sites]]` is the right tool.

## Worked Example

Shoppr’s app manifest declares:

```toml
[[sites]]
id = "shoppr-uk"
display_name = "Shoppr UK"
brand_name = "Shoppr"
canonical_domain = "uk.localhost"
additional_domains = ["www.localhost", "shop.example.com", "www.example.com"]
default_locale = "en-GB"
supported_locales = ["en-GB", "fr-FR", "pl-PL"]

[[sites]]
id = "shoppr-fr"
display_name = "Shoppr France"
brand_name = "Shoppr Paris"
canonical_domain = "fr.localhost"
default_locale = "fr-FR"
supported_locales = ["en-GB", "fr-FR", "pl-PL"]
```

That means:

- requests to the UK hosts resolve the UK site first
- requests to the French host resolve the French site first
- the same locale set is available across sites
- the default locale changes by site
- branding can differ by site even though the product family is still one app

## When A Locale Is Enough

Add a locale when:

- the hostname stays the same
- the site identity stays the same
- the product battery stays the same
- the main change is language and regional presentation

Examples:

- English and French versions of the same UK store
- localized copy for one global product with shared inventory and routing logic

## When You Need A Site

Add a site when:

- the hostname changes meaningfully
- the brand display differs
- the default locale differs by market
- inventory, pricing, promotions, or operational policy diverge
- editorial or SEO behaviour should be anchored to a different site identity

Examples:

- UK, France, and Poland under one brand
- one product with distinct market hosts and merchandising

## A Practical Decision Matrix

Use a locale when:

- you are translating one site
- you want alternate localised routes under the same product surface

Use a site when:

- you are modelling a distinct market-facing delivery surface
- you need different hostnames or brand identity
- you need distinct default locale behaviour

Use a separate app only when:

- the product is no longer really the same application
- module battery, deployment, auth, or business identity diverge so far that sharing one customer app stops making sense

## Why This Matters For Developers

This model prevents a common failure mode where "multi-language support" is treated as template text replacement while the rest of the application still assumes:

- a single host
- a single assortment
- a single canonical route graph

Coil is trying to keep those concerns aligned from the start.

## Common Mistakes

### Treating site as only a hostname

A site is broader than a host binding. It is part of the application model.

### Treating locale as only translation

Locale affects routing, metadata, and content shape, not just strings.

### Treating market as a synonym for site

Sometimes they line up. Often they do not. Keeping them distinct makes commerce behaviour more composable.

### Using sites where simple locales would be enough

That creates unnecessary host and routing complexity.

## Read Next

- [Request and render lifecycle](../request-and-render-lifecycle/)
- [app.toml](../reference/app-toml/)
- [Shoppr use case overview](../use-cases/shoppr/overview/)
- [Gitly use case overview](../use-cases/gitly/overview/)
