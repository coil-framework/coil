# Site, Brand, And Locale Resolution

**Part:** Customer Apps  
**Chapter:** 97

## Status

Accepted.

## Decision

Davenda supports a first-class `site` model inside a single customer app.

A customer app may expose:

- one brand with one site
- one brand with multiple sites
- multiple sites sharing modules, templates, auth, and operational runtime while varying public-host, locale, branding, and merchandising behavior per site

`locale` remains a presentation concern. It is not the primary boundary for regional commerce.

The core model is:

- `customer app`
  - the deployable implementation boundary
- `site`
  - the public commercial/editorial boundary inside one customer app
- `locale`
  - the language and formatting layer inside a site
- `brand`
  - shared identity defaults with optional site overrides

The first production slice standardizes `site` and allows brand-facing overrides directly on the site record. A separate shared brand registry can be added later if customer demand justifies it.

## Why

Locale alone is not enough for real multi-country or multi-region storefronts. Real customer implementations often need:

- different inventory availability
- different events and launch calendars
- different pricing or currencies
- different legal or SEO hosts
- different editorial emphasis
- different site-level brand presentation

If those concerns are forced into locale, the model becomes incoherent:

- content and commerce rules get mixed together
- route and SEO policy become ambiguous
- inventory and catalog visibility cannot be expressed cleanly

Sites solve that without forcing customers into separate deployments for every country or region.

## Model

### Customer App Manifest

The customer app manifest may declare `[[sites]]`.

Each site declares:

- `id`
- `display_name`
- `brand_name`
- canonical domain plus additional domains
- `default_locale`
- `supported_locales`

The existing top-level `domains` and `i18n` sections remain the app-level compatibility/default layer:

- they are still valid for single-site apps
- they remain the default site/global fallback in mixed environments
- site records may narrow locale support and host routing within those app-level bounds

### Runtime Config

Platform config may declare `[[sites]]`.

Each runtime site record declares:

- `id`
- `display_name`
- `brand_name`
- `canonical_host`
- `hosts`
- `default_locale`
- `supported_locales`

Global `i18n` and `seo` settings remain valid and continue to describe:

- app-wide defaults
- the compatibility path for single-site deployments
- the fallback host/locale policy when no explicit site is matched

### Request Resolution

Davenda resolves site before locale-sensitive route matching.

The resolution order is:

1. determine the active site from the request host
2. determine locale using the matched site's supported locales
3. resolve the route
4. build execution context with `customer_app`, `site`, and `locale`

This keeps host, locale, canonical URL generation, and rendering aligned.

### Rendering And Hooks

The request execution context exposes site identity to:

- templates
- linked customer Rust hooks
- runtime SEO generation
- storefront/catalog selection logic

The customer SDK must therefore expose site identity as a stable first-party field, not as an internal runtime leak.

## Consequences

### Positive

- customers can model UK, US, AU, DE style storefronts inside one customer app
- Shoppr and similar examples can demonstrate host-selected multi-site behavior cleanly
- locale remains focused on translation and formatting
- SEO and canonical host generation stay coherent
- customer-linked Rust hooks can apply site-aware business rules without separate services

### Negative

- more configuration must be validated for uniqueness and alignment
- runtime request resolution becomes site-aware
- customer templates and sample apps must stop assuming a single hard-coded brand/site name

## Non-Goals For This Slice

This decision does not, by itself, require:

- separate deployments per site
- separate auth tenants per site
- a global reusable brand registry
- full price-list or tax-engine separation

Those can be layered later if needed.

## Shoppr Demonstration Requirements

Shoppr must demonstrate at least three sites inside one customer workspace.

The demo must show:

- three different site ids bound to different hosts
- site-specific branding or editorial emphasis
- site-specific catalog or event visibility
- site-aware rendering and navigation without forking the whole app into three separate deployments
