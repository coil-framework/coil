# SEO, Metadata, Sitemaps, and JSON-LD

SEO is a platform concern because search-facing behavior cuts across routing, rendering, publishing, localization, and content ownership. If metadata is left as ad hoc template markup, customer apps and official modules will duplicate logic, structured data will drift, and publication workflows will forget to invalidate the things search engines actually consume.

## A Typed Metadata Model

Core should own a typed head and metadata API. Routes and handlers register title, description, canonical URL, robots directives, Open Graph or social metadata, and any other document-level signals through a structured model that the renderer resolves when building the final page. Templates consume that resolved state; they do not invent it independently.

This matters because metadata depends on real platform context. Canonical URLs are locale-aware. Robots behavior can depend on environment or publish state. Open Graph images may come from the managed media system. Once those concerns are typed and centralized, official modules and customer apps can contribute metadata safely without rebuilding the head from scratch.

## Structured Data Is Built, Not Hand-Assembled

The same principle applies to JSON-LD. Core provides a typed builder and validation rules. Official modules contribute schema nodes for the resources they own: organization and website metadata from the CMS shell, breadcrumb trails from navigation, product and offer data from commerce, and event data from the events module. Customer apps supply the customer-specific content that fills those nodes, such as brand identity, copy, and image selection.

Using a typed builder is important for the same reason the template engine is constrained. Search-facing behavior is too important to leave to raw string concatenation. It should be possible to test that a published event page emits the expected `Event` node, that an offer has price and availability in the correct locale context, and that a private or draft resource does not leak into structured data accidentally.

## Sitemaps and Publication

Sitemap generation is also part of core. Modules contribute entries for the resource types they own, and the customer app's publish policy decides which of those entries are eligible for inclusion. Locale variants, canonical relationships, change timestamps, and image or alternate-language metadata should be derived from the same publishing model used by page rendering.

Because publication is a state transition, sitemap and metadata invalidation must be tied to the same events and cache keys used elsewhere. Publishing a page, event, or product should update the rendered head, the JSON-LD output, the relevant sitemap entries, and any CDN or application-cache state that might otherwise continue to serve stale search signals.

## Extension and Review Boundaries

WASM extensions may contribute metadata, sitemap entries, and typed structured-data nodes through host APIs, but they should not inject arbitrary `<head>` markup or hand-built schema blobs into pages. The platform needs to remain able to analyze, validate, and test the search-facing output of the composed application.

That is the real contract of SEO on this platform. Search behavior is treated as part of the application model, not as an editorial afterthought layered onto whatever markup happened to be produced.
