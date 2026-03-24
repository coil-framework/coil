# HTTP Caching and CDN Integration

**Part:** Data and Storage  
**Chapter:** 37

Application caching and HTTP caching serve different purposes and the platform needs both. The internal cache stack reduces repeated computation inside or across application nodes. HTTP caching controls how full responses and fragments are reused by browsers, reverse proxies, and CDNs. Treating them as the same thing would make it impossible to reason clearly about public delivery.

Core therefore owns response-level primitives such as `ETag`, `Last-Modified`, `Cache-Control`, variation keys, and surrogate-style invalidation tags. Official native modules and customer apps use those primitives to describe whether a response is public, private, tenant-specific, locale-specific, user-specific, or not cacheable at all. The runtime and any edge layer then apply those semantics consistently.

Public pages are the main beneficiaries. A product page, article, or public event page can usually be cached at the edge with variation by site and locale, while still being invalidated when publication state or related content changes. Personalized account pages, carts, and admin responses are different; they generally stay private or no-store even if internal caches help compute their contents. Fragment endpoints follow the same rule. A fragment is not automatically safe to edge-cache just because it is smaller than a full document.

CDN integration is designed around predictable invalidation rather than guesswork. Hashed static assets are immutable and rely on their fingerprinted URLs, so they rarely need active purging. HTML responses and managed public assets are different. They may use surrogate keys or equivalent tags tied to resource identity so a page publish, event update, or asset unpublish can trigger precise invalidation without resorting to broad cache clears.

The locale and auth systems both feed into HTTP cache behavior. Locale influences canonical URLs, `hreflang`, and variation keys. Auth state determines whether a response can be treated as public at all. A response that includes publication-controlled media or customer-specific pricing must declare that explicitly. This is why the platform puts cache semantics in core rather than leaving them to whichever team wrote the template.

Customer apps choose their CDN and proxy topology, but they do so against a stable contract. Whether the edge is Cloudflare or another reverse proxy, the application should emit response semantics that remain meaningful and testable without vendor-specific template hacks.
