---
title: Internationalization, Localization, And Content
---

Davenda treats locale as part of request context, not as a helper called from templates after the real work is finished.

That matters because locale affects much more than copy.

## What Locale Touches

Locale can affect:

- route resolution
- canonical and alternate URL generation
- text translation
- date, time, number, and money formatting
- SEO metadata
- cache variation
- sometimes which content or product data is valid to publish

This is why internationalization is documented as a framework concern rather than as a frontend utility.

## Locale Resolution

The request path is site-aware first and locale-aware second.

At a high level:

1. the runtime resolves the site from the request host
2. it resolves the locale within that site’s supported locale policy
3. route matching, rendering, metadata, and formatting all use the same resolved context

This avoids one of the most common multi-lingual failure modes: templates, routers, and metadata all disagreeing about which locale the user is actually viewing.

## Translation Keys Versus Content

Davenda separates two related but different things:

- **translated UI strings** such as navigation labels, action copy, and system messages
- **localized content** such as product descriptions, CMS page copy, or localized slugs

The first usually belongs in message catalogs or translation dictionaries.

The second belongs in managed data or content models, not in configuration files.

## Translation Keys In Templates

Templates should be written so that user-visible copy can be translated without duplicating the whole page for each locale.

In practice that means:

- keeping reusable text in translation dictionaries where possible
- passing explicit localized values in the render model when the content is managed data
- avoiding assumptions that the current route, host, and locale will always be the same

## Locale Fallback

A fallback locale is a resilience tool, not a substitute for publication discipline.

Good uses:

- default formatting behavior
- temporary UI-string fallback

Bad uses:

- silently serving untranslated customer content forever
- treating fallback as a reason not to publish real localized content

## Common Mistakes

### Treating locale as only text replacement

Locale affects routing, metadata, and cache behavior too.

### Putting translated content into config

Product content belongs in content or data workflows, not in `platform.toml`.

### Confusing site and locale

Sites choose the commercial or editorial boundary. Locales choose the language and formatting layer within that boundary.

## What To Read Next

- [Sites, Locales, And Markets](./sites-locales-and-markets.md)
- [Internationalization Reference](../reference/internationalization.md)
- [SEO Reference](../reference/seo.md)
