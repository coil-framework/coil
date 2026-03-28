---
title: SEO And Discoverability
---

Davenda treats SEO as part of the application model because discoverability depends on routing, locale, rendering, publication, and metadata all staying aligned.

## Why SEO Is Not Just Head Markup

Search-facing behavior depends on:

- canonical host resolution
- locale-aware URLs
- structured metadata
- sitemap generation
- robots behavior
- stable page rendering

If those are handled ad hoc in templates, they drift quickly.

## Typed Metadata Over String Assembly

Davenda’s design expects routes and handlers to contribute metadata through structured models rather than by every page hand-building its own `<head>` logic.

That makes it possible to keep:

- titles
- descriptions
- canonical URLs
- alternate locale relationships
- JSON-LD
- environment-specific robots policy

coherent across modules and customer pages.

## Why This Matters For Multi-Site Apps

Multi-site apps have an extra SEO burden because:

- different sites may have different canonical hosts
- different locales may map to different URL variants
- some content may be published in one site or locale but not another

The runtime needs to know the active site and locale before SEO output can be correct.

## What To Read Next

- [Internationalization, Localization, And Content](./internationalization-localization-and-content.md)
- [SEO Reference](../reference/seo.md)
- [Build And Deploy](../operations/build-and-deploy.md)
