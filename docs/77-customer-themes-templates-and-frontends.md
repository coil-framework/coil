# Customer Themes, Templates, and Frontends

**Part:** Customer Apps  
**Chapter:** 77

The customer app is where brand, UX, and content presentation become concrete. The platform is deliberately SSR-first, with progressive enhancement layered on top, so customer frontends are built from server-rendered templates, fragments, and asset bundles rather than from a mandatory client-heavy application shell.

## Template Model

The platform's template direction is HTML-first and intentionally constrained. Templates should remain recognizable HTML documents with attribute-driven expressions, fragments, includes, slots, loops, and conditionals, but without turning into a scripting language.

That fits customer apps well because it supports:

- storefront pages
- account and membership flows
- event and booking journeys
- branded CMS pages
- fragment-based partial updates for progressively enhanced interactions

## Theming Boundaries

Themes should own:

- layout structure
- brand styling and design tokens
- presentation-level fragment overrides
- customer copy, imagery, and localized content

Themes should not reach into module internals to replace business rules. If a customer needs new business behavior, that belongs in configuration, module selection, or a WASM extension, not in a template hack.

## Assets and Delivery

Compiled theme and site assets are deployment artifacts. They should be hashed, published to object storage or CDN at build or deploy time, and treated as public once released. This is different from managed customer assets such as media or downloadable files, which may be governed by auth and storage policy.

Because the frontend is SSR-first, the asset pipeline should favor durable CSS, small progressive-enhancement scripts, and stable fragment rendering over large client-side rehydration payloads.

## Admin Frontend Limits

Customer branding may influence the admin shell, but only within supported boundaries. The platform still needs accessible tables, forms, dialogs, focus handling, and predictable operator workflows. Customer-specific admin needs should generally arrive through documented widgets and module composition rather than by replacing the admin shell wholesale.

## Multi-Brand and Locale Context

Customer apps can still support multiple brands or regional experiences through the core site, brand, locale, and theme primitives. The key is that those concerns are expressed as customer-app composition, not by forking core rendering behavior.
