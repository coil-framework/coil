# Localization, Internationalization, and Locale Routing

Internationalization is a core service because locale affects far more than strings. It changes routing, formatting, SEO, cache variation, theme selection, legal copy, and sometimes even the product behavior a customer app chooses to expose. The platform therefore treats locale as part of request context, not as a helper library called from templates after the real work is done.

## Locale Resolution Produces a Full Request Context

At the start of a request, core resolves the active locale using customer-app policy. That policy can consider hostname, path prefix, stored preference, and request headers, but the result is a single locale context the rest of the stack can trust. The locale context should include at least language, region, timezone, and currency so rendering, pricing, date formatting, and metadata generation all speak the same language.

This is especially important in a platform that supports multiple customer apps, white-label sites, and region-aware experiences. Locale cannot be allowed to drift independently in templates, payment logic, and SEO helpers. One request, one resolved context.

## Routing and URLs Are Locale-Aware

The router and URL generator must understand locale because localized applications need predictable addressability. Customer apps should be able to choose path-based, host-based, or mixed locale routing according to their product requirements, but once chosen, the framework should generate canonical URLs, alternate URLs, and internal links consistently. A locale-aware page should never rely on a hand-built string in a template.

Localized slugs and content fallbacks belong to the same model. Official modules should support translated system UI and, where appropriate, localized fields or slugs. Customer apps define which locales they publish in, which fields can fall back, and when missing translations should block publication instead of silently borrowing another locale's content.

## Translation and Formatting

Core should provide message catalog loading, fallback chains, and ICU-style formatting for dates, numbers, money, and pluralization. Official modules ship their own translatable UI copy. Customer apps provide brand-specific copy, editorial content, and any locale policy that affects the customer experience. WASM extensions can contribute translation bundles only through declared host contracts so the runtime can continue to analyze completeness and load the right catalogs for a request.

The distinction between translated copy and formatted data matters. A bookings screen may render translated labels, event times in the viewer's timezone, and prices in the active currency. Those are related, but they are not the same problem, so the platform keeps them in the same request context rather than scattering them across unrelated helpers.

## Cache, SEO, and Publishing Consequences

Locale must participate in cache keys for rendered documents, fragments, and any localized lookup data. It also drives `hreflang`, canonical generation, sitemap partitioning, and structured-data output. A page published only in French is not the same cache object or sitemap entry as its English equivalent, and the system should model that explicitly.

The first reference customer app makes this concrete. Event and membership pages may need localized copy, timezone-aware times, currency-specific pricing language, and locale-specific metadata. If the locale system is merely an afterthought in templates, the rest of the platform cannot remain correct. Treating locale as a core request primitive avoids that failure mode.
