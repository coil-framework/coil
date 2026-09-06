---
title: SEO
---

Coil SEO is runtime-owned metadata built from route, site, and locale context.

## Start With The Output

On a normal public page, Coil can inject markup like this into the document head:

```html
<meta name="description" content="..." />
<link rel="canonical" href="https://gitly.example.com/fr/explore" />
<meta name="robots" content="index,follow" />
<link rel="alternate" hreflang="en-GB" href="https://gitly.example.com/explore" />
<link rel="alternate" hreflang="fr-FR" href="https://gitly.example.com/fr/explore" />
<meta property="og:title" content="..." />
<script type="application/ld+json">...</script>
```

That is the right mental model:

- templates own visible structure
- the runtime owns the search-facing metadata envelope

## What Is Configured?

Current checked-in SEO config looks like this:

```toml
[seo]
canonical_host = "gitly.example.com"
emit_json_ld = true
```

And it works together with i18n config such as:

```toml
[i18n]
default_locale = "en-GB"
supported_locales = ["en-GB", "fr-FR", "de-DE"]
fallback_locale = "en-GB"
localized_routes = true
```

Because canonical and alternate URLs are route- and locale-aware, SEO cannot be treated as a
completely separate subsystem.

## Field Reference

### `canonical_host`

- Required: yes in current checked-in platform configs
- Type: host string
- Meaning: default canonical host for absolute URL generation

Interaction:

- site-specific canonical hosts override the app-level default when a site is resolved

### `emit_json_ld`

- Required: no
- Type: boolean
- Meaning: whether the runtime should emit built-in JSON-LD page metadata

## What Is Automatic Today?

Coil currently generates these pieces automatically at the document boundary:

- meta description
- canonical URL
- robots meta
- alternate `hreflang` links
- Open Graph title, description, and type
- baseline JSON-LD page nodes when enabled

Important practical behaviour:

- if the page already has `</head>`, the runtime injects before it
- if the page has no `<head>`, the runtime creates one

This is why templates do not need to re-implement head assembly page by page.

## What Is Customizable?

The runtime can merge route- or handler-provided metadata into the automatic baseline.

Current custom inputs include:

- explicit title
- explicit description
- explicit canonical URL
- extra alternate URLs
- extra robots directives
- extra JSON-LD nodes

The extension point is typed metadata from runtime code, not hand-built strings in templates.

## Canonical And Alternate URL Logic

Coil builds canonical and alternate URLs from:

- resolved site
- site canonical host
- route name
- route params
- route locale policy
- supported locales for that site

So for a localised route:

1. the site resolves from the request host
2. the locale resolves from the route and site policy
3. the runtime emits the canonical URL for that exact route/site/locale
4. the runtime emits alternates only for equivalent localised routes

That is why hardcoding canonical links in templates is almost always the wrong move.

## JSON-LD

Current behaviour:

- if `emit_json_ld = true`, the runtime emits a page-level JSON-LD node automatically
- extra JSON-LD nodes can be appended through typed metadata

This is the correct place for:

- page schema
- product schema
- event schema
- structured metadata that belongs to the route, not to incidental template layout

## Common Mistakes

### Building canonical URLs by string concatenation

Use runtime site-aware route generation instead.

### Forgetting site and locale when reasoning about metadata

That is the fastest way to produce wrong-host or wrong-language head output.

### Treating private or draft content like public indexed content

SEO output should follow actual publication state.

### Rebuilding `<head>` behaviour inside every template

That defeats the whole runtime-owned metadata model.

## Supporting Implementation And Repo Examples

Concrete supporting files:

- `apps/shoppr/platform.toml`
- `apps/shoppr/platform.dev.toml`
- `apps/gitly/platform.toml`
- `apps/gitly/platform.dev.toml`
- `crates/coil-runtime/src/render/seo.rs`
- `crates/coil-runtime/src/http/routing/model.rs`
- `crates/coil-runtime/src/render/model.rs`

## What Should I Read Next?

- [Internationalisation](./internationalization/)
- [Themes, Rendering, And Assets](../core-concepts/themes-rendering-and-assets/)
- [SEO And Discoverability](../core-concepts/seo-and-discoverability/)
