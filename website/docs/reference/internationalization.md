---
title: Internationalisation
---

Davenda internationalisation starts at request resolution, not in a browser-only translation
helper.

## Start With The Two Real Patterns

Current Davenda apps use two honest patterns, but they sit on top of richer server-side runtime
primitives than the demos currently make obvious.

### Pattern 1: server-rendered localised values

```html
<html xmlns:dv="https://davenda.dev" dv:attr="lang=${locale}">
  <h1 dv:text="${page.title}">Fallback title</h1>
  <p dv:text="${account.stateSummary}">Fallback summary</p>
</html>
```

This is the right path for:

- first render
- transactional pages
- SEO-relevant copy
- module-owned surfaces

### Pattern 2: customer-owned translation-key dictionaries

```html
<h1 data-i18n="home.title">One Davenda app can look like a forge.</h1>
<button type="button" data-i18n-control="dark">Dark</button>
```

This is the checked-in Gitly pattern for:

- theme controls
- demo copy
- app-owned frontend strings

The key distinction is important:

- Davenda resolves locale at the runtime level
- a translation-key dictionary is currently a customer convention, not a built-in template API

## What Already Exists In Core

Davenda already has server-side i18n runtime primitives in core:

- locale tags
- locale contexts
- fallback chains
- translation catalogs
- translation runtime lookup
- locale-aware URL routing

So the platform is not limited to browser-only translation.

What is still missing from the public customer-facing story is narrower:

- a first-class customer translation file convention
- a template-native translation helper
- a checked-in demo that wires customer translation catalogs into server-rendered page copy

## What Is Configured?

At the app level:

```toml
[i18n]
default_locale = "en-GB"
supported_locales = ["en-GB", "fr-FR", "pl-PL"]
localized_routes = true
```

At the runtime level:

```toml
[i18n]
default_locale = "en-GB"
supported_locales = ["en-GB", "fr-FR", "pl-PL"]
fallback_locale = "en-GB"
localized_routes = true
```

At the site level:

```toml
[[sites]]
id = "shoppr-fr"
default_locale = "fr-FR"
supported_locales = ["en-GB", "fr-FR", "pl-PL"]
```

## Field Reference

### `default_locale`

- Required: yes
- Type: locale tag string
- Meaning: default locale for the app or site

### `supported_locales`

- Required: yes
- Type: array of locale tag strings
- Meaning: locales the app or site is willing to serve

### `localized_routes`

- Required: yes in current checked-in manifests
- Type: boolean
- Meaning: whether locale is part of the route contract

### `fallback_locale`

- Required: runtime-level field in current checked-in configs
- Type: locale tag string
- Meaning: fallback behaviour for runtime locale handling

## How Request Resolution Works

Davenda resolves:

1. site from the host
2. locale inside that site’s locale policy
3. route under the resulting site-and-locale context

That keeps these things aligned:

- localised URLs
- render values
- canonical URLs
- alternate locale links

The practical outcome is simple:

- templates should use `locale`, `site.*`, and `links.*`
- templates should not hand-build locale-prefixed paths

## What Templates Can Read

The base request model includes values such as:

- `locale`
- `site.id`
- `site.displayName`
- `site.brandName`
- `site.canonicalHost`
- `links.*`

Typical usage:

```html
<html xmlns:dv="https://davenda.dev" dv:attr="lang=${locale}">
  <a dv:attr="href=${links.home}">
    <span dv:text="${site.brandName}">Brand</span>
  </a>
</html>
```

That is the correct template boundary. Locale-aware runtime values are already shaped before the
template runs.

## Translation Files And Dictionaries Today

Current honest state:

- Davenda does not yet ship a framework-owned customer translation file format
- Davenda does not yet ship a template-native `t("key")` helper
- Gitly demonstrates a customer-owned locale dictionary in frontend JS
- Shoppr demonstrates server-rendered, locale-aware values, multi-site locale configuration, and
  route-aware server-rendered market and locale switch URLs

So if you need translation keys today, define a customer-owned convention and document it clearly.

## Key Naming Pattern

A stable pattern looks like this:

- page prefix: `home`, `search`, `explore`, `actions`
- field suffix: `title`, `summary`, `empty`
- grouped controls: `controls.language`, `controls.theme`
- grouped navigation: `nav.home`, `nav.profile`

That is exactly the pattern Gitly uses.

## Adding A New Locale

The practical sequence is:

1. add the locale to app-level `supported_locales`
2. add it to the relevant site’s `supported_locales`
3. decide whether the site’s `default_locale` changes
4. update customer-owned translation dictionaries if you use them
5. update localised content and server-rendered copy
6. verify localised routes and SEO output

If the host, brand, or assortment also changes, you probably need a new site, not just a new locale.

## Common Mistakes

### Pretending there is already a built-in translation-file system and `t()` helper

There is not. Be explicit about the current customer convention.

### Mistaking the Gitly `site.js` dictionary for the platform boundary

It is a demo choice, not the architectural limit of Davenda.

### Hardcoding `/en-GB/` or `/fr/` paths in templates

Use runtime-generated links instead.

### Treating locale as only text replacement

Locale also affects routes and SEO.

### Confusing site with locale

Site is the public brand and host boundary. Locale is the language and formatting layer inside it.

## Supporting Implementation And Repo Examples

Concrete supporting files:

- `apps/shoppr/app.toml`
- `apps/shoppr/platform.toml`
- `apps/shoppr/platform.dev.toml`
- `apps/shoppr/catalog.toml`
- `apps/gitly/app.toml`
- `apps/gitly/platform.toml`
- `apps/gitly/platform.dev.toml`
- `apps/gitly/theme/assets/site.js`
- `crates/davenda-runtime/src/http/routing/model.rs`
- `crates/davenda-runtime/src/render/seo.rs`

## What Should I Read Next?

- [Template Models](./template-models.md)
- [SEO](./seo.md)
- [Themes, Rendering, And Assets](../core-concepts/themes-rendering-and-assets.md)
- [Internationalisation, Localisation, And Content](../core-concepts/internationalization-localization-and-content.md)
