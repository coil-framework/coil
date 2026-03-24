# The Template Engine

**Part:** Rendering and Frontend  
**Chapter:** 24

The first-party template engine is a Rust-native, HTML-aware renderer designed specifically for server-rendered applications. It is intentionally closer in spirit to Thymeleaf than to Velocity: the primary artifact is a valid HTML document, not a general-purpose text script with markup mixed into it.

## Design Goals

The engine exists to support storefronts, CMS pages, account areas, and admin interfaces that remain readable, cacheable, and safe under heavy reuse. Templates should therefore stay close to ordinary HTML. Most dynamic behavior is expressed through attributes and named fragments, not through large inline control blocks that turn the template into a second programming language.

That leads to a few hard rules. Escaping is on by default. Expression support is intentionally limited. Layout inclusion, conditional rendering, iteration, slot injection, localized message lookup, and safe attribute binding are built in because they are necessary for real applications. Dynamic evaluation, arbitrary code execution, and direct data access from templates are not. A template cannot reach into the database, inspect global service state, or open a network client just because the syntax would allow it.

## Not a Literal Thymeleaf Port

The platform should adopt the ergonomics of HTML-first templating without copying another engine wholesale. The goal is a stricter Rust implementation with better compile-time diagnostics, stronger typing around view models, and clearer fragment boundaries. Templates are parsed into an HTML-aware AST and compiled into render plans the runtime can cache aggressively. Errors should point to a template path, line, and failing expression rather than surfacing as opaque runtime panics.

This is also why the HTML engine is not forced to solve every text-generation problem. HTML, XML-like metadata documents, and email markup can share the same design principles where appropriate, but the system should not contort the HTML engine into the universal DSL for code generation, raw text email, and arbitrary config output. If the platform eventually needs a simpler text templating mode, that should be a separate surface.

## Data Flow Into Templates

Controllers and handlers prepare explicit view models. Those view models are the only data templates can see. The model may expose typed helpers for URLs, localization, money formatting, asset lookup, or capability-driven UI decisions, but the template still reads data instead of orchestrating business rules. If a decision requires database access, auth expansion, or storage inspection, it belongs back in the handler or domain service.

This separation matters across all platform layers. Core owns the renderer and shared helpers. Official modules own their template namespaces and the view-model shapes they publish. Customer apps may override allowed templates, supply additional fragments, and inject theme tokens or brand-specific layouts. WASM extensions participate only through host-approved render APIs. They do not get to invent ad hoc syntax or bypass escaping rules.

## Fragments, Slots, and Partial Rendering

The engine is built with fragment rendering as a first-class concept, because fragment responses are central to the platform's progressive-enhancement model. A fragment is not just an include; it is a named renderable unit with a defined input model, cache behavior, and placement contract. Slots let layouts and higher-level fragments inject content without giving the template system arbitrary mutation semantics.

This supports the real UI patterns the platform needs: shared headers and footers, admin shell chrome, event cards, booking panels, cart summaries, validation summaries, modal content, and inline dashboard widgets. The same fragment should be usable inside a full document render and as the payload of a fragment endpoint without changing its semantics.

## Operational Consequences

Because templates remain constrained and HTML-aware, the platform can lint them, analyze dependencies, watch them in development, and cache them safely in production. Template override order can be made deterministic. Cache keys can incorporate locale, site or brand, theme selection, and auth scope. Accessibility and SEO helpers can be integrated directly into the rendering pipeline instead of depending on convention alone.

That is the core architectural reason for this engine. It is not trying to be a clever language feature. It is the part of the platform that keeps UI authoring expressive without allowing the view layer to become another source of runtime entropy.
