---
title: Internationalisation
---

Davenda resolves locale on the server, but the public apps currently demonstrate two different
copy-delivery patterns on top of that runtime model.

Use this page to keep those patterns straight:

- Shoppr shows server-resolved locale, host-aware sites, and locale-aware links
- Gitly shows customer-owned frontend dictionaries on top of localized routes

## The Runtime Part Is Already Real

Shoppr’s manifest is the shortest concrete example:

```toml
[i18n]
default_locale = "en-GB"
supported_locales = ["en-GB", "fr-FR", "pl-PL"]
localized_routes = true

[[sites]]
id = "shoppr-fr"
canonical_domain = "fr.localhost"
default_locale = "fr-FR"
supported_locales = ["en-GB", "fr-FR", "pl-PL"]
```

That contract means the runtime already resolves:

1. site from the host
2. locale inside that site
3. route and links under that site-and-locale context

So locale is not a browser-only afterthought.

## Pattern 1: Server-Resolved Locale Values

This is the base pattern templates should prefer for request-critical and SEO-relevant output:

```html
<html xmlns:dv="https://davenda.dev" dv:attr="lang=${locale}">
  <a dv:attr="href=${links.home}">
    <span dv:text="${site.brandName}">Brand</span>
  </a>
</html>
```

That is the Shoppr-style boundary:

- the runtime resolves `locale`
- the runtime shapes `links.*`
- the template consumes already-localized routing context

Use this pattern for:

- alternate locale links
- page shells
- account and admin surfaces
- checkout and confirmation pages

## Pattern 2: Customer-Owned Frontend Dictionaries

Gitly intentionally uses a narrower, app-owned pattern for copy:

```html
<h1 data-i18n="actions.title">Workflow runs</h1>
<p data-i18n="actions.mockBody">
  This browser-side loop simulates a scheduled refresh so the Actions demo shows visible cadence.
</p>
```

And its frontend script applies the dictionary after the page is rendered:

```js
function applyCopy(locale) {
  const messages = translations[locale] || translations["en-GB"];
  document.querySelectorAll("[data-i18n]").forEach((node) => {
    const key = node.getAttribute("data-i18n");
    const value = messages.copy[key] || messages[key];
    if (value) node.textContent = value;
  });
}
```

That demonstrates a real customer choice, not a platform limit.

Use this pattern when:

- the strings are product-shell or demo copy
- the app wants to own the dictionary format entirely
- client-side hydration is acceptable

Do not mistake it for “the Davenda i18n API.” It is Gitly’s chosen implementation.

## What Davenda Does Not Yet Ship As A Customer API

Current honest state:

- there is no built-in customer translation file convention
- there is no template-native `t("key")` helper
- the public demos do not yet wire a checked-in customer translation catalog into server-rendered
  page copy

So if you need translation dictionaries today, define them in customer code and document that
choice clearly.

## What To Copy Right Now

### If you need server-first locale behavior

Copy the Shoppr pattern:

- declare locales and sites in `app.toml`
- declare runtime locale policy in `platform.dev.toml`
- consume `locale`, `site.*`, and `links.*` in templates

### If you need app-owned UI copy dictionaries

Copy the Gitly pattern:

- keep localized routes in app/runtime config
- keep dictionary keys in app-owned assets
- apply those strings in frontend code
- document clearly that this is a customer convention

## Common Mistakes

### Claiming Gitly is “server-rendered translated copy”

It is not. Gitly’s route/locale resolution is runtime-backed, but the visible translated copy is
currently applied in frontend JS.

### Hardcoding locale-prefixed paths in templates

Use runtime-generated links instead.

### Treating locale as only a text problem

Locale also affects routes, canonical URLs, and alternate links.

### Confusing site with locale

Site is the host/brand/public-surface boundary. Locale is the language/formatting layer inside
that surface.

## Read Next

- [SEO](./seo.md)
- [Template Models](./template-models.md)
- [Gitly Theming, Localization, And Accessibility](../use-cases/gitly/theming-localization-and-accessibility.md)
- [Sites, locales, and markets](../core-concepts/sites-locales-and-markets.md)
