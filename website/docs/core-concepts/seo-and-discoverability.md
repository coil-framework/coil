---
title: SEO And Discoverability
---

This page explains how Davenda’s routing, site, locale, and rendering model combine into search and
discoverability behavior.

## What Is This?

Davenda treats SEO as part of the application model rather than as scattered `<head>` snippets in
templates.

The important moving parts are:

- site resolution
- locale-aware routes
- canonical and alternate URLs
- robots policy
- Open Graph metadata
- JSON-LD

## Why Does It Exist?

Search-facing correctness depends on the same core facts as the rest of the app:

- which host is canonical
- which locale the route is serving
- whether equivalent localized routes exist
- whether the content is public

If those are guessed in templates, they drift.

## When Should Developers Think About It?

Whenever they:

- add a public route
- add a new site
- add a new locale
- add event, product, or editorial pages
- decide whether a page should be indexed

## What Is Automatic Today?

Davenda’s render layer currently injects:

- meta description
- canonical URL
- robots meta
- alternate `hreflang` links
- Open Graph fields
- baseline JSON-LD page nodes when enabled

That behavior lives in:

- `crates/davenda-runtime/src/render/seo.rs`

This is the core conceptual shift:

- templates own the visible document structure
- the runtime owns the search-facing metadata envelope

## What Is Custom Today?

Routes and handlers can still extend the automatic baseline with typed metadata such as:

- explicit title
- explicit description
- explicit canonical override
- extra alternate URLs
- extra robots directives
- extra JSON-LD

So the extension point is typed runtime metadata, not handwritten string assembly in every page.

## Worked Example: Canonical, Alternates, Robots, And JSON-LD

Use Gitly as the compact example.

Config:

```toml
[i18n]
default_locale = "en-GB"
supported_locales = ["en-GB", "fr-FR", "de-DE"]
fallback_locale = "en-GB"
localized_routes = true

[seo]
canonical_host = "gitly.example.com"
emit_json_ld = true
```

Files:

- `apps/gitly/platform.toml`
- `apps/gitly/platform.dev.toml`

What happens on a public localized page:

1. the runtime resolves the site from the host
2. it resolves the locale from the URL and site policy
3. it computes the canonical absolute URL for that site and locale
4. it computes alternate `hreflang` URLs for the supported locales on that site
5. it injects robots, Open Graph, and JSON-LD into the document head

That is the concrete “automatic” path.

## Why Multi-Site Apps Need This

Shoppr is the strongest example here.

Relevant files:

- `apps/shoppr/app.toml`
- `apps/shoppr/platform.toml`
- `apps/shoppr/platform.dev.toml`

Shoppr demonstrates:

- one customer app
- multiple sites
- different canonical hosts per site
- localized routes
- site-aware catalog availability

Without a site-aware SEO model, those pages would quickly emit the wrong canonical host or wrong
alternate locale set.

## How Site And Locale Affect Discoverability

Davenda resolves site before locale-sensitive routing.

That matters because:

- canonical host must come from the resolved site
- alternate locales must come from the resolved site’s supported locale set
- localized route generation must match the actual route policy

This is why SEO, site resolution, and internationalization cannot be documented as separate
unrelated topics.

## What Templates Should And Should Not Do

Templates should:

- set `<html lang>` correctly from `locale`
- use runtime-generated links
- stay focused on document structure and user-visible content

Templates should not:

- hand-build canonical URLs
- guess alternate locale URLs
- duplicate head metadata logic page by page

## Constraints And Common Mistakes

### Building canonical URLs by string concatenation

Use route and site aware runtime generation instead.

### Forgetting site and locale in metadata reasoning

That is how wrong-host and wrong-language metadata leaks into production.

### Treating private or draft content like public indexed content

Discoverability should follow publication state.

### Assuming JSON-LD is entirely template-owned

The current runtime model is designed to inject it from typed metadata and configured behavior.

## What Should I Read Next?

- [SEO](../reference/seo.md)
- [Internationalization](../reference/internationalization.md)
- [Themes, Rendering, And Assets](./themes-rendering-and-assets.md)
- `crates/davenda-runtime/src/render/seo.rs`
