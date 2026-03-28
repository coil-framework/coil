---
title: SEO
---

This page documents the concrete SEO surface Davenda supports today.

## What Is This?

Davenda’s SEO model is the combination of:

- canonical host selection
- localized route-aware canonical and alternate URLs
- head metadata injection
- robots policy
- Open Graph metadata
- JSON-LD emission

It is runtime-owned. Templates should not guess at it by string concatenation.

## Why Does It Exist?

SEO correctness depends on the same inputs as routing and rendering:

- route name
- route params
- site
- locale
- publication state

If canonical URLs, alternates, and metadata are composed manually in templates, they drift quickly.

## When Should I Use It?

Use the SEO model whenever a page is:

- public
- localized
- served on a canonical host
- intended to be indexed or shared

That includes storefront, editorial, product, and event pages.

## Which Exact Files And Settings Are Involved?

Customer-facing config:

- `platform.toml`
- `platform.dev.toml`

Current runtime knobs:

```toml
[seo]
canonical_host = "gitly.example.com"
emit_json_ld = true
```

Runtime implementation:

- `crates/davenda-runtime/src/render/seo.rs`
- `crates/davenda-runtime/src/http/routing/model.rs`
- `crates/davenda-runtime/src/render/model.rs`

Checked-in examples:

- `apps/shoppr/platform.toml`
- `apps/shoppr/platform.dev.toml`
- `apps/gitly/platform.toml`
- `apps/gitly/platform.dev.toml`

## Field Reference

### `canonical_host`

- Required: yes in current checked-in platform configs
- Type: host string
- Meaning: default canonical host when the runtime builds absolute URLs

Interaction with sites:

- site-specific canonical hosts override the app-level default when a site is resolved

### `emit_json_ld`

- Required: no, but explicitly set in the checked-in demos
- Type: boolean
- Meaning: whether the runtime should inject built-in JSON-LD page metadata

## What Is Automatic?

Davenda currently generates these things automatically at the document boundary:

- meta description
- canonical URL
- robots meta
- alternate `hreflang` links
- Open Graph title, description, and type
- baseline JSON-LD page node when enabled

This is implemented in `crates/davenda-runtime/src/render/seo.rs`.

Important behavior:

- if `</head>` exists, metadata is injected before it
- if there is no `<head>`, the runtime creates one before `<body>`
- if there is no `<body>`, the runtime prepends a `<head>` block

## What Is Customizable?

Davenda’s render layer can also merge route- or handler-provided metadata into the automatic
baseline.

Current supported metadata extensions include:

- explicit title
- explicit description
- explicit canonical URL
- extra alternate URLs
- extra robots directives
- extra JSON-LD nodes

That means the right extension point is typed metadata from runtime code, not hand-built head markup
inside page templates.

## Canonical URLs

Davenda builds canonical URLs from:

- resolved site
- site canonical host
- route name
- route params
- locale policy

In multi-site apps, canonical host follows the matched site.

In localized routes, canonical and alternate URLs follow the active locale model.

Practical rule:

- never hardcode canonical links in templates unless you are deliberately overriding runtime output

## Alternate Locale URLs

Davenda only emits alternate locale URLs for routes that are actually localized.

That behavior comes from:

- route locale policy
- site-supported locales
- the current resolved site

This is why SEO and i18n must be documented together.

## Worked Example

Gitly config:

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

What the runtime does on a localized page:

1. resolve the active site from the host
2. resolve the locale from the localized route
3. build the canonical URL for that site and locale
4. emit alternate `hreflang` URLs for the supported locales on that site
5. inject title, description, robots, Open Graph, and JSON-LD

## JSON-LD

Current baseline behavior:

- if `emit_json_ld = true`, the runtime emits a page-level JSON-LD node automatically
- extra JSON-LD nodes can be added through typed metadata

Use this for:

- page schema
- product schema
- event schema
- other structured nodes supplied by runtime code

Do not build large JSON strings directly in templates unless there is no better typed path yet.

## Common Mistakes

### Building canonical URLs by string concatenation

Use site-aware route generation instead.

### Forgetting locale and site in metadata reasoning

That is how wrong-host and wrong-language metadata leaks into production.

### Treating draft or private content like public indexed content

SEO output should follow publication state.

### Re-implementing `<head>` generation in every template

That defeats the whole point of runtime-owned metadata.

## What Should I Read Next?

- [Internationalization](./internationalization.md)
- [Template Language](./template-language.md)
- [SEO And Discoverability](../core-concepts/seo-and-discoverability.md)
- `crates/davenda-runtime/src/render/seo.rs`
