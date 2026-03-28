---
title: SEO
---

Davenda treats SEO as a typed, runtime-aware concern rather than as scattered ad hoc template markup.

This page explains the practical pieces developers need to keep aligned.

## The Main SEO Outputs

Davenda apps should reason explicitly about:

- document title and description
- canonical URL
- alternate locale URLs
- robots policy
- Open Graph or social metadata
- JSON-LD
- sitemap entries

These are not independent features. They all depend on the same route, site, locale, and publication model.

## Canonical Hosts

Canonical host policy must align with site resolution.

If a page is served on multiple hosts, the canonical choice should come from the active site model or explicit SEO configuration, not from template guesswork.

## Locale-Aware SEO

Multi-lingual apps need:

- canonical URLs that reflect the active locale policy
- alternate locale links where equivalent content exists
- locale-aware sitemap entries
- localized metadata where appropriate

This is why Davenda’s SEO model is tightly connected to its i18n model.

## Structured Data

Use structured metadata intentionally. Product, event, breadcrumb, organization, and website schema should come from typed data and route-aware context rather than hand-built JSON strings where possible.

The design goal is that the platform can validate and test search-facing output instead of treating it as invisible page decoration.

## Sitemaps

Sitemaps should reflect:

- current publication state
- locale availability
- canonical relationships
- the actual public route inventory

They should not be a separate manually curated artifact that drifts from the application.

## Common Mistakes

### Building canonical URLs by string concatenation

Use the site and route model instead.

### Forgetting locale and site in cache or metadata logic

This is a common source of wrong-language or wrong-host metadata.

### Treating draft or private content like public content

SEO output should follow publication state, not bypass it.

## What To Read Next

- [SEO And Discoverability](../core-concepts/seo-and-discoverability.md)
- [Internationalization Reference](./internationalization.md)
- [Shoppr Sites, Locales, And Theme Variants](../use-cases/shoppr/sites-locales-and-theme-variants.md)
