# Example Commerce Installation

**Part:** Appendices  
**Chapter:** 94

This appendix describes a commerce-focused installation intended for a mid-sized online storefront. It is the reference shape for customers who need catalog, checkout, order management, CMS pages, media, and an admin interface, but do not need the events-and-memberships vertical.

## Module Composition

The installation composes the following official modules:

- `commerce-catalog`
- `commerce-checkout`
- `commerce-orders`
- `commerce-payments-stripe`
- `cms-pages`
- `media-library`
- `admin-shell`
- `admin-content`
- `admin-commerce`

Core provides the runtime, auth engine, storage, caching, i18n, SEO, TLS, and WASM hosting beneath them all.

## Reference Install Choices

```toml
[modules]
enabled = [
  "commerce-catalog",
  "commerce-checkout",
  "commerce-orders",
  "commerce-payments-stripe",
  "cms-pages",
  "media-library",
  "admin-shell",
  "admin-content",
  "admin-commerce",
]

[cache]
l1 = "moka"
l2 = "redis"

[storage]
default_class = "public_upload"
```

The customer app still owns theme, copy, translations, and brand presentation, but the runtime and product behavior come from the composed platform layers.

## Storefront Model

The storefront is server-rendered by default. Product detail pages, category pages, landing pages, and account pages are full HTML responses with progressive enhancement used for cart updates, filters, and small account interactions. This keeps SEO, performance, and memory usage aligned with the platform’s SSR-first design.

JSON APIs still exist, but they are reserved for integrations and the cases where a fragment-oriented HTML response is not the right fit.

## Cache And Delivery Policy

The installation uses two cache layers:

- `moka` as in-process L1 for hot route and lookup caching
- `redis` or `valkey` as distributed L2 for shared invalidation, locks, and coordinated cache state

Cache scoping is explicit:

- category and product pages are usually public but scoped by locale, currency, and site
- account pages, carts, and checkout are private or uncacheable
- CMS fragments are cacheable only when they do not depend on session or user capability state

Writes from catalog publishing, price changes, or content updates should invalidate product pages, collection listings, sitemaps, structured data fragments, and affected navigation entries together.

## Storage And Assets

Storage policy is typical of a public storefront:

- build assets use `public_asset` and publish through the deploy pipeline to object storage and CDN
- product images and marketing media use `public_upload`
- invoices, exports, and support attachments use `private_shared`
- rare sensitive uploads can opt into `local_only_sensitive`, but this should be exceptional because it complicates multi-node deployments

Managed media publication is still auth-governed. A product image may exist in object storage before it is publicly visible.

## Auth Model

The default auth package is usually sufficient for a straightforward commerce install. Common bindings include:

- merchandisers who can edit products and collections
- content editors who can manage pages and SEO metadata
- finance or support staff who can read orders and issue refunds without broader catalog privileges
- administrators who can manage modules and runtime policy

Official modules consume capabilities such as `catalog.product.edit`, `order.read`, and `order.refund.issue`, which keeps the install compatible even if the customer later replaces the default relation graph.

## SEO And Localization

The storefront enables:

- locale-aware routes and translated catalog content
- canonical URLs and redirect handling
- JSON-LD for `Organization`, `WebSite`, `Product`, `Offer`, and `BreadcrumbList`
- per-locale titles, descriptions, and open graph metadata

SEO is not an afterthought layered onto templates. It is emitted through the platform’s typed metadata and structured-data services so modules and customer templates stay consistent.

## Operational Notes

A commerce installation should validate at least the following before launch:

- payment-provider credentials and webhook verification
- cache scoping around account, cart, and checkout paths
- media publication rules for public and restricted assets
- object-store and CDN behavior under rollout
- refund, order lookup, and audit trails in admin

This installation is a good baseline for most commerce customers because it exercises the full storefront stack while still preserving the platform boundary between core, official modules, and app-owned frontend composition.
