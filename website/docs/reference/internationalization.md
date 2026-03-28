---
title: Internationalization
---

Davenda internationalization is built around request context, localized routing, translation lookup, and stable fallback rules.

This page explains the practical contract for developers building multi-lingual apps.

## The Main Pieces

At minimum, internationalization in Davenda involves:

- app-level locale policy in `app.toml`
- runtime locale defaults in `platform.toml`
- site-specific locale support through `[[sites]]`
- translation dictionaries or message catalogs
- localized template output
- localized routes and URLs

## App-Level Locale Configuration

The app manifest provides the product-facing locale contract:

```toml
[i18n]
default_locale = "en-GB"
supported_locales = ["en-GB", "fr-FR", "pl-PL"]
localized_routes = true
```

Use this to describe the locales the customer app intends to support.

## Site-Level Locale Configuration

For multi-site apps, each site narrows or specializes locale policy:

```toml
[[sites]]
id = "shoppr-fr"
default_locale = "fr-FR"
supported_locales = ["en-GB", "fr-FR", "pl-PL"]
```

This is how one customer app can support different hosts with different primary locale expectations.

## Runtime Locale Configuration

`platform.toml` adds deployment-level defaults such as:

- `fallback_locale`
- runtime `localized_routes` behavior
- site-level runtime host bindings

Use this for environment and runtime behavior, not for the source-of-truth product definition.

## Translation Keys In Templates

Davenda templates should be written so UI strings can be translated without duplicating whole page files for each locale.

Two common patterns are:

- render-model values already localized by Rust code
- template or enhancement-layer hooks that look up translated strings by key

In the checked-in demos, Gitly uses a client-side translation dictionary for some UI copy, while Davenda’s broader model still treats locale as a first-class server request concern.

## Localized Routes

When `localized_routes = true`, the route system is expected to preserve locale as part of the URL contract rather than as hidden state.

That matters for:

- shareable URLs
- SEO
- canonical generation
- cache keys

## Fallback Rules

A fallback locale is a resilience tool, not a substitute for publication discipline.

Good uses:

- default formatting behavior
- temporary UI-string fallback

Bad uses:

- silently serving untranslated customer content forever
- treating fallback as a reason not to publish real localized content

## Common Mistakes

### Putting translation strategy only in frontend code

Davenda’s locale model starts at request resolution, not in a browser toggle alone.

### Treating localized routes as optional decoration

They are part of the page identity, not an afterthought.

### Using locale where site is the real boundary

If the catalog, host, or merchandising differs, use a site.

## What To Read Next

- [Sites, Locales, And Markets](../core-concepts/sites-locales-and-markets.md)
- [SEO Reference](./seo.md)
- [Shoppr Sites, Locales, And Theme Variants](../use-cases/shoppr/sites-locales-and-theme-variants.md)
