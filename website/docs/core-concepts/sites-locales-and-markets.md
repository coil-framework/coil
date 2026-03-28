---
title: Sites, Locales, And Markets
---

Davenda treats site and locale as runtime concerns. They are not just view-layer labels.

If you skip this distinction, multi-site behavior becomes confusing quickly.

## What It Is

These three terms are related but different:

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

If those distinctions are flattened together, route resolution and commerce behavior become much harder to reason about.

## How It Works

In Davenda, the request path first resolves the site from the host and then applies locale-aware route matching within that site context.

That lets the runtime carry site and locale through:

- route resolution
- canonical URL generation
- feature flag evaluation
- render-model assembly
- product visibility checks

Markets then layer on top where commerce rules need them.

## Why This Matters For Developers

This model prevents a common failure mode where "multi-language support" is treated as template text replacement while the rest of the application still assumes a single host, a single assortment, and a single canonical route graph.

Davenda is trying to keep those concerns aligned from the start.

## Common Mistakes

### Treating site as only a hostname

A site is broader than a host binding. It is part of the application model.

### Treating locale as only translation

Locale affects routing, metadata, and content shape, not just strings.

### Treating market as a synonym for site

Sometimes they line up. Often they do not. Keeping them distinct makes commerce behavior more composable.

## What To Read Next

- [Request and render lifecycle](request-and-render-lifecycle.md)
- [Shoppr use case overview](../use-cases/shoppr/overview.md)
- [Gitly use case overview](../use-cases/gitly/overview.md)
