---
title: Internationalization
---

This page documents the internationalization and localization model Davenda supports today.

## What Is This?

Internationalization in Davenda is the combination of:

- app-level locale configuration
- runtime fallback and host-aware locale behavior
- site-specific locale support
- localized route generation
- localized render-model values
- customer-owned translation key conventions where needed

It is a request and rendering concern, not just a frontend helper.

## Why Does It Exist?

Locale affects much more than copy:

- route identity
- canonical and alternate URLs
- formatting
- cache variation
- SEO metadata
- language-specific UI

If locale is treated as a pure browser toggle, the app and the runtime drift apart quickly.

## When Should I Use It?

Use the Davenda i18n model whenever your app needs:

- more than one locale
- localized URLs
- different site defaults for different markets or regions
- correct canonical and alternate URL generation

If the product is single-locale today, still keep the locale settings explicit. That makes later
expansion much safer.

## Which Exact Files Are Involved?

The concrete files are:

- app locale contract: `app.toml`
- runtime locale and SEO defaults: `platform.toml`, `platform.dev.toml`
- request resolution: `crates/davenda-runtime/src/http/routing/model.rs`
- SEO alternate and canonical generation: `crates/davenda-runtime/src/render/seo.rs`
- base render model values: `crates/davenda-runtime/src/render/model.rs`

Checked-in app examples:

- `apps/shoppr/app.toml`
- `apps/shoppr/platform.toml`
- `apps/shoppr/platform.dev.toml`
- `apps/gitly/app.toml`
- `apps/gitly/platform.toml`
- `apps/gitly/platform.dev.toml`

## App-Level Locale Configuration

The customer app manifest defines the product-facing locale contract:

```toml
[i18n]
default_locale = "en-GB"
supported_locales = ["en-GB", "fr-FR", "pl-PL"]
localized_routes = true
```

Field guidance:

### `default_locale`

- Required: yes
- Type: locale tag string
- Meaning: the default locale for the app or site

### `supported_locales`

- Required: yes
- Type: array of locale tag strings
- Meaning: which locales the app or site is willing to serve

### `localized_routes`

- Required: yes in app manifests today
- Type: boolean
- Meaning: whether localized routes are part of the URL contract

## Runtime Locale Configuration

The runtime config adds deployment-level behavior:

```toml
[i18n]
default_locale = "en-GB"
supported_locales = ["en-GB", "fr-FR", "pl-PL"]
fallback_locale = "en-GB"
localized_routes = true
```

Extra runtime field:

### `fallback_locale`

- Required: yes in current runtime configs
- Type: locale tag string
- Meaning: the fallback locale for request and rendering behavior

Use runtime config for environment behavior, not for replacing the customer app’s source-of-truth
product definition.

## Site-Level Locale Configuration

Multi-site apps declare site-specific locale behavior with `[[sites]]`.

Example from Shoppr:

```toml
[[sites]]
id = "shoppr-fr"
display_name = "Shoppr France"
brand_name = "Shoppr Paris"
canonical_domain = "fr.127.0.0.1.nip.io"
default_locale = "fr-FR"
supported_locales = ["en-GB", "fr-FR", "pl-PL"]
```

This is how one customer app can serve:

- English-first UK traffic
- French-first French traffic
- Polish-first Polish traffic

without becoming three separate deployments.

## How Request Resolution Works

The runtime resolves:

1. site from the host
2. locale from that site’s supported locale policy
3. route under the resulting site-and-locale context

This behavior lives in `crates/davenda-runtime/src/http/routing/model.rs`.

Practical consequence:

- do not hardcode locale prefixes in templates
- do not assume locale can be chosen without site context

## What Templates Receive

Templates receive these concrete values from the base render model:

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

Use those values instead of hand-built locale paths or brand strings.

## Translation Files And Dictionaries

This is the most important honesty point on current HEAD:

Davenda does **not** yet ship a first-class framework-owned translation file format with a
template-native lookup helper.

Current checked-in patterns are:

### Server-rendered localized values

Use runtime code to bind already-localized strings into the render model, then render them with
normal template bindings.

This is the best fit for:

- transactional surfaces
- SEO-relevant content
- module-owned pages
- forms, errors, and first-render UI

### Customer-owned theme-side dictionaries

Gitly currently demonstrates a theme-side translation dictionary in:

- `apps/gitly/theme/assets/site.js`

That file contains locale-keyed dictionaries such as:

- `controls.language`
- `home.title`
- `search.empty`

And templates use attributes such as:

- `data-i18n`
- `data-i18n-nav`
- `data-i18n-control`
- `data-i18n-aria-label`

Examples:

- `apps/gitly/templates/gitly/home.html`
- `apps/gitly/templates/gitly/explore.html`
- `apps/gitly/templates/gitly/search.html`

That is a valid customer-app convention, but it is not a framework-native translation API.

## Key Naming Patterns

If you adopt a translation-key dictionary today, keep keys boring and stable.

Gitly’s pattern is a good model:

- page or area prefix: `home`, `search`, `explore`, `actions`
- subfield suffix: `title`, `summary`, `empty`
- control groups: `controls.language`, `controls.dark`
- navigation groups: `nav.home`, `nav.profile`

Good examples:

- `home.title`
- `search.empty`
- `controls.theme`
- `nav.actions`

Bad examples:

- keys that encode HTML structure
- keys with random abbreviations
- keys that mix site, locale, and control state into one unstable identifier

## Template Translation Examples

### Server-rendered example

```html
<h1 dv:text="${page.title}">Fallback title</h1>
<p dv:text="${account.stateSummary}">Fallback summary</p>
```

This is the dominant pattern in Shoppr.

### Theme-dictionary example

```html
<h1 data-i18n="home.title">One Davenda app can look like a forge.</h1>
<button type="button" data-theme-option="dark" data-i18n-control="dark">Dark</button>
```

This is the checked-in Gitly pattern.

## Fallback Behavior

Fallback locale is a resilience tool, not a publication strategy.

Good uses:

- default formatting behavior
- temporary UI-string fallback
- route generation when the app needs a stable default

Bad uses:

- silently serving untranslated product content forever
- using fallback as an excuse not to publish localized content

## English, French, And Polish Shoppr Example

Shoppr is the canonical multi-site example:

- `shoppr-uk`
  - default locale `en-GB`
- `shoppr-fr`
  - default locale `fr-FR`
- `shoppr-pl`
  - default locale `pl-PL`

Key files:

- `apps/shoppr/app.toml`
- `apps/shoppr/platform.toml`
- `apps/shoppr/platform.dev.toml`
- `apps/shoppr/catalog.toml`

What this shows:

- one customer app
- three sites
- site-specific branding
- localized routes
- site-aware catalog availability

## Common Mistakes

### Pretending there is a built-in `t()` helper today

There is not. Document your customer-app convention honestly.

### Hardcoding `/en-GB/` or `/fr/` paths

Use runtime-generated links instead.

### Confusing site with locale

If the host, brand, or assortment differs, that is usually a site boundary, not just a locale.

### Putting customer content into config files

Config should describe locale policy, not become a CMS replacement.

## What Should I Read Next?

- [Template Language](./template-language.md)
- [Theme Structure](./theme-structure.md)
- [SEO](./seo.md)
- [Internationalization, Localization, And Content](../core-concepts/internationalization-localization-and-content.md)
- `apps/shoppr/app.toml`
- `apps/gitly/theme/assets/site.js`
