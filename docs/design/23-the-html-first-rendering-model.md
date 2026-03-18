# The HTML-First Rendering Model

**Part:** Rendering and Frontend  
**Chapter:** 23

The platform is built around server-rendered HTML because the primary workloads are storefronts, account areas, CMS pages, admin screens, membership journeys, and event or booking flows. Those are document-oriented experiences. They need strong SEO, predictable accessibility, low memory use, and operational simplicity more than they need a client-side application runtime on every page.

## HTML Is the Default Artifact

Core treats HTML documents and HTML fragments as first-class response types. A normal page request resolves a route, loads a typed view model, renders a full document, and emits metadata, cache hints, and structured data in the same response. Enhanced interactions follow the same philosophy: they usually fetch another HTML fragment, not a JSON payload that asks the browser to rebuild the DOM from scratch.

This choice keeps the platform aligned with its business shape. Product pages, event listings, booking flows, admin tables, and account settings all benefit from being correct on first response, crawlable before JavaScript runs, and easy to cache at both the application and CDN layers. It also keeps the runtime lean. The system avoids hydration-heavy architectures that spend CPU and memory reconstructing a page the server already knew how to render.

## Interactivity Is Layered on Top

HTML-first does not mean static. It means the server remains the authority for business state and document structure. Interactivity is layered on through fragment endpoints, progressively enhanced forms, and small client-side controllers where they genuinely improve the experience. The platform deliberately prefers HTML-over-the-wire updates for things like filters, cart summaries, inline admin tables, booking availability panels, and media pickers.

JSON is still supported, but it is explicit and secondary. It exists for machine-facing APIs, integrations, and genuinely client-heavy widgets. It is not the default transport for ordinary page behavior. This matters because once every interaction depends on bespoke JSON contracts, the server-rendered model becomes a thin shell around an accidental SPA, and the platform loses most of the simplicity it was designed to preserve.

## Responsibilities Across the Stack

Core owns the rendering contract: response types, template invocation, fragment semantics, cache variation, locale-aware URL generation, head metadata registration, and the rules for partial rendering. Official modules own semantic UI for the domain they serve. The events module renders event pages, timeslot selectors, and booking fragments. The CMS module renders content pages, redirects, and navigation-aware layouts. The admin shell renders forms, tables, filters, and dialogs using the same platform primitives.

Customer apps own branding, layout composition, content model choices, installed modules, and the final template overrides allowed by those modules. They do not replace the rendering model itself. WASM extensions can participate by registering routes, fragment handlers, metadata providers, or UI contributions through host APIs, but they do not introduce a competing client framework or a second template engine. The native host remains responsible for rendering guarantees, auth checks, cache safety, and accessibility obligations.

## A Page Request Through the Model

Consider an event detail page in the first reference customer app. The full document route resolves the site or brand, locale, theme, and user context; loads the event, timeslot summary, SEO metadata, and publication state; checks the relevant capabilities; and renders a complete HTML page. The booking panel on that page may later refresh independently as capacity changes or the user selects a date, but the refresh is still an HTML fragment rendered by the same templates and governed by the same auth, locale, and cache rules.

That is the intended shape of the platform: documents first, fragments second, JSON only where justified, and no hidden promotion of the rendering layer into an unbounded application runtime.
