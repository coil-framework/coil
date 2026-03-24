# Reverse Proxies, CDNs, and Edge Integration

**Part:** Operations  
**Chapter:** 69

The platform is designed to run both directly at the edge and behind external reverse proxies or CDNs. In either case, the application still owns canonical URL logic, auth-aware cache decisions, and the rules for how assets and managed files are served. Edge infrastructure accelerates delivery; it does not replace the platform's security or content model.

## Trusted Edge Metadata

When a proxy or CDN sits in front of the application, the platform must reconstruct the original request from trusted headers only. That requires an explicit trusted-proxy allowlist. The application should not blindly trust arbitrary forwarded headers from the public internet.

Correct proxy metadata matters for:

- secure cookie and redirect behavior
- canonical URL generation and SEO metadata
- scheme-aware link generation
- signed URL validation and app-proxy routing
- audit logs and rate limiting

## HTTP Caching Model

Core provides the cache primitives; the edge consumes them. The important headers and behaviors are:

- `ETag` and `Last-Modified` for validator-based caching
- `Cache-Control` for freshness and personalization boundaries
- tag or surrogate-key style invalidation for page, fragment, and sitemap rebuilds
- explicit separation between public, locale-scoped, tenant-scoped, user-scoped, session-scoped, and uncacheable responses

Personalized responses must never leak into shared edge cache. The platform's auth-aware cache scope model is the safeguard here.

## Assets and Managed Files

Static theme and site assets are deployment artifacts. They should be built into hashed bundles, published to object storage or CDN at build or deploy time, and treated as always public once released.

Managed assets follow storage policy and auth state:

- `public_cdn` for publicly publishable files
- `signed_url` for direct private downloads from object storage
- `app_proxy` when the application must enforce access on every request
- `local_only` only for explicitly exceptional cases

An asset being stored in object storage does not automatically make it public. Publication is an auth-governed state transition.

## Edge Invalidation

First-party modules and approved extensions can emit cache invalidation intent, but the platform owns invalidation semantics. A page publish, product update, or event cancellation should invalidate the related page fragments, navigation, sitemap entries, and structured-data output through one consistent mechanism rather than ad hoc CDN scripts per module.

## Practical Deployment Rule

The clean deployment stance is:

- let the application define cacheability and canonical semantics
- let the CDN accelerate public and semi-static traffic
- keep private delivery behind signed URLs or app proxying
- never let proxy configuration become an alternate auth system

That preserves the platform's content, auth, and storage model even when the edge topology varies by customer app.
