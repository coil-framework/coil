---
title: Request And Render Lifecycle
---

Davenda is an HTML-first framework. That sentence is easy to repeat and easy to misunderstand.

The important point is that request handling, auth, route resolution, render-model assembly, and progressive enhancement all belong to one coherent lifecycle.

## What It Is

The request and render lifecycle is the path from an incoming HTTP request to:

- a full HTML page
- a fragment update
- a redirect after a form action
- a typed JSON response for a genuinely API-shaped route

Davenda treats full pages as the default path, not as a thin fallback after API design.

## Why It Exists

Many web stacks split the product in awkward ways:

- a browser app owns most state
- server rendering is optional or bolted on
- forms and redirects feel secondary
- auth and route semantics drift between page and API layers

Davenda tries to keep those concerns unified, because real products usually mix:

- public pages
- account surfaces
- admin pages
- stateful form actions
- localized routes

## How It Works

The exact internals are deeper than this page, but the shape is consistent:

1. the runtime resolves the request against host, site, locale, and route surfaces
2. auth and capability checks run against the resolved route
3. request input is normalized and validated
4. handlers execute page, action, or API behavior
5. a render model is assembled for page-shaped responses
6. templates render HTML using explicit data rather than arbitrary code execution

That lifecycle is what lets Davenda keep HTML-first rendering without giving up operational or security discipline.

## What "HTML-First" Means In Practice

It means:

- forms and redirects are normal
- server-rendered pages are normal
- fragments are supported for progressive enhancement
- JSON exists when the route is truly API-shaped

It does not mean:

- no interactivity
- no JavaScript
- no typed backend behavior

## Common Mistakes

### Thinking of rendering as a late presentation step

Site, locale, auth, and module composition all affect rendering. It is not just "turn data into HTML."

### Expecting arbitrary logic in templates

Davenda keeps templates deliberately constrained. Complex state should be prepared in Rust render models, not improvised inside the template engine.

### Treating form actions as second-class behavior

In Davenda, stateful form flows are part of the primary model, especially for storefronts, account areas, and admin surfaces.

## What To Read Next

- [Sites, locales, and markets](sites-locales-and-markets.md)
- [Customer Rust vs third-party WASM](../reference/customer-vs-wasm.md)
- [Reference overview](../reference/overview.md)
