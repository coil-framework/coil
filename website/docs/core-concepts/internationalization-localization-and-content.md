---
title: Internationalisation, Localisation, And Content
---

This page explains how locale, localised routes, translated UI, and customer content actually fit
together in Coil.

## What Is This?

Coil treats internationalisation as a combination of:

- request-time locale resolution
- site-aware locale policy
- localised routes
- translated UI strings
- localised content and formatting

Those are related, but they are not the same problem.

## Why Does It Exist?

Real customer apps need locale to influence more than copy:

- routing
- canonical and alternate URLs
- formatting
- search-facing metadata
- cache keys
- customer-visible controls

If locale is handled only in frontend code, the application model becomes incoherent.

## When Should I Use This Model?

Use this model whenever you:

- add a second locale
- add a new site with a different default locale
- decide whether copy should live in a translation dictionary or in localised content
- choose between frontend translation keys and server-rendered localised strings

## How Locale Resolution Works

Coil resolves:

1. site from host
2. locale inside that site’s allowed locale set
3. route under that site-and-locale context

That means route matching, render values, and SEO all agree about what the user is actually seeing.

The runtime code for that lives in:

- `crates/coil-runtime/src/http/routing/model.rs`

## Translation Dictionaries Versus Localised Content

Keep these separate.

### Translation dictionaries

Use for:

- nav labels
- button text
- control labels
- small explanatory UI strings

Today’s checked-in example is Gitly’s theme-side dictionary in:

- `apps/gitly/theme/assets/site.js`

### Localised content

Use for:

- CMS content
- product descriptions
- account messaging produced by runtime code
- SEO-relevant content bodies

This should come from managed content or render-model values, not from a frontend dictionary.

## Server-Rendered I18n Versus Demo Translation Dictionaries

Current honest state:

- Coil core already ships server-side locale primitives, locale contexts, fallback chains,
  locale-aware URL routing, translation catalogs, and a translation runtime
- Coil does not yet ship a first-class customer-facing translation file convention plus a
  template-native translation helper
- Gitly demonstrates a customer-owned theme-side dictionary in `apps/gitly/theme/assets/site.js`
- Shoppr demonstrates server-rendered locale-aware values, site-aware rendering, and now
  server-shaped route-aware market and locale switch targets

So if you ask “can Coil support server-rendered i18n?”, the answer is yes.

If you ask “does the current public demo show a full customer translation catalog loaded into
templates on the server?”, the honest answer is no, not yet.

That distinction matters:

- the platform primitives exist
- the current checked-in demo translation story is still incomplete
- Gitly's `site.js` is a demo convention, not the framework limit

## Key Naming Patterns

If you adopt a translation-key dictionary today, use stable semantic keys.

Gitly’s checked-in pattern is a good model:

- page or area prefix: `home`, `explore`, `search`, `actions`
- grouped control keys: `controls.language`, `controls.dark`
- grouped navigation keys: `nav.home`, `nav.actions`

Examples from `apps/gitly/theme/assets/site.js`:

- `home.title`
- `home.summary`
- `search.empty`
- `controls.theme`
- `nav.profile`

## Template Translation Examples

### Server-rendered value

```html
<h1 coil:text="${page.title}">Fallback</h1>
<p coil:text="${account.state_summary}">Fallback summary</p>
```

This is the right pattern for first-render, transactional, and SEO-relevant copy.

### Customer-owned translation-key convention

```html
<h1 data-i18n="home.title">One Coil app can look like a forge.</h1>
<button type="button" data-i18n-control="dark">Dark</button>
```

This is the checked-in Gitly pattern for theme and demo UI strings. It is not the only possible
Coil i18n model, and it should not be mistaken for the full platform contract.

## Fallback Examples

Fallback locale is useful for:

- temporary UI-string fallback
- stable locale defaults
- route generation when a localised path needs a deterministic default

Fallback locale is not a substitute for:

- publishing localised customer content
- product translation discipline

Current runtime configs show this explicitly:

- `apps/shoppr/platform.toml`
- `apps/shoppr/platform.dev.toml`
- `apps/gitly/platform.toml`
- `apps/gitly/platform.dev.toml`

## English, French, And Polish Shoppr Example

Shoppr is the canonical multi-site, multi-locale example.

Relevant files:

- `apps/shoppr/app.toml`
- `apps/shoppr/platform.toml`
- `apps/shoppr/platform.dev.toml`
- `apps/shoppr/catalog.toml`

What it demonstrates:

- `shoppr-uk` with default locale `en-GB`
- `shoppr-fr` with default locale `fr-FR`
- `shoppr-pl` with default locale `pl-PL`
- one customer app
- site-aware branding
- localised routes
- site-specific availability in `catalog.toml`

This is the example to follow when adding a new locale and deciding whether it should also be a new
site.

## How To Add A New Locale

The practical sequence is:

1. add the locale to app-level `supported_locales`
2. add it to the appropriate site’s `supported_locales`
3. decide whether the site’s `default_locale` should change
4. update customer-owned translation dictionaries if you use them
5. update server-rendered localised content or data if the page content is localised
6. verify localised routes and canonical behaviour in the running app

If the host, brand, or assortment also changes, you likely need a new site, not just a new locale.

## Constraints And Common Mistakes

### Pretending the framework already owns translation-file format and lookup

It does not. Document the customer convention honestly.

### Treating locale as only text replacement

Locale also affects routing and metadata.

### Putting customer content into config files

Config describes locale policy. It should not become your CMS.

### Confusing site with locale

Sites choose public brand and host boundary. Locales choose language and formatting within that
boundary.

## What Should I Read Next?

- [Internationalisation](../reference/internationalization/)
- [SEO](../reference/seo/)
- [Sites, Locales, And Markets](./sites-locales-and-markets/)
- `apps/shoppr/app.toml`
- `apps/gitly/theme/assets/site.js`
