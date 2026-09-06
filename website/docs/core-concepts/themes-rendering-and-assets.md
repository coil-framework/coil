---
title: Themes, Rendering, And Assets
---

This page explains how Coil turns a customer-owned theme into a final HTML document with working
assets and injected metadata.

## Start With The Full Path

A single page render usually passes through these layers:

```text
request
  -> route + site + locale resolution
  -> render model assembly
  -> template lookup through namespaces
  -> asset-path resolution through the active manifest
  -> SEO/head decoration
  -> final HTML response
```

That is the real mental model. Themes are part of the render pipeline, not just decorative files.

## What A Theme Contributes

A Coil theme contributes four practical things:

- document structure
- reusable fragments
- published frontend assets
- customer-owned presentation behaviour such as theme mode or small enhancements

This is why the theme is broader than “the CSS folder.”

## One Concrete Document Flow

Imagine a customer layout like this:

```html
<!DOCTYPE html>
<html xmlns:coil="https://coil.rs" coil:attr="lang=${locale}">
  <head>
    <title coil:text="${page.title}">Fallback title</title>
    <link rel="stylesheet" href="/theme/assets/site.css" coil:href="asset('theme/assets/site.css')" />
  </head>
  <body>
    <nav coil:replace="~{navigation/primary}"></nav>
    <main coil:slot="content"></main>
    <script src="/theme/assets/site.js" coil:src="asset('theme/assets/site.js')" defer="defer"></script>
  </body>
</html>
```

What happens:

- `locale` already comes from request resolution
- `page.title` already comes from the render model
- `asset('theme/assets/site.css')` resolves to the published asset URL
- `coil:replace` pulls in a fragment
- the runtime later injects canonical, robots, alternate locale links, and JSON-LD into the head

That one example is the entire subsystem in miniature.

## Why Some Templates Carry Full HTML Structure

This surprises people coming from frameworks that hide the outer document shell.

Coil keeps full HTML structure in customer templates because:

- the customer app owns the actual product shell
- the customer app often owns the nav, header, footer, and landmarks
- SEO and asset references are still part of customer-facing page composition

So seeing `<html>`, `<head>`, and `<body>` in customer templates is normal and correct.

## Layouts, Fragments, Pages, And Assets

### Layouts

Layouts own:

- document shell
- slots
- global navigation or footer
- shared page furniture

### Fragments

Fragments own:

- reusable sections
- nav blocks
- collection grids
- account summary panels

### Pages

Pages own:

- route-specific content
- headings
- page-level forms and lists

### Assets

Assets own:

- CSS
- enhancement JS
- images and icons

The important separation is:

- templates describe structure
- assets describe presentation and enhancement

## Asset Publication And Hashed Delivery

Customer templates should reference logical asset names:

```html
coil:href="asset('theme/assets/site.css')"
coil:src="asset('theme/assets/site.js')"
```

Coil then:

1. publishes assets from the declared theme asset roots
2. gives them hashed artifact paths
3. records the active manifest
4. injects logical-path to public-URL mappings into the render model

That keeps templates stable while allowing production-safe cache busting.

## Head Metadata And JSON-LD Injection

Document head output is not only what the template wrote by hand.

After the template renders, the runtime can inject:

- description
- canonical URL
- robots
- alternate locale links
- Open Graph fields
- JSON-LD

This is why themes and SEO belong in one conceptual conversation. The customer owns the visible
document shell, but the runtime owns the search-facing metadata contract.

## Where Accessibility Fits

A theme is not successful if it looks branded but breaks semantics.

At the theme level, the app still owns:

- landmarks
- skip links
- visible focus
- contrast
- usable language and theme controls

That is why rendering, assets, and accessibility are tightly coupled in Coil.

## Common Mistakes

### Treating the theme as only CSS

The theme includes templates, assets, and the customer-owned shell.

### Rebuilding application state in `site.js`

Enhancement scripts should improve the HTML-first path, not replace it.

### Hardcoding final asset URLs in templates

Use logical asset names and the asset helper instead.

### Assuming SEO metadata must be hand-authored in every page template

The runtime already has a metadata decoration stage.

## Supporting Implementation And Repo Examples

Concrete supporting files:

- `apps/shoppr/templates/layouts/base.html`
- `apps/shoppr/templates/layouts/storefront.html`
- `apps/shoppr/templates/pages/home.html`
- `apps/gitly/templates/gitly/home.html`
- `apps/gitly/theme/assets/site.js`
- `crates/coil-runtime/src/render/model.rs`
- `crates/coil-runtime/src/render/seo.rs`
- `crates/coil-assets/src/release.rs`

## What Should I Read Next?

- [Theme Structure](../reference/theme-structure/)
- [Theme Asset Delivery](../reference/theme-asset-delivery/)
- [Template Models](../reference/template-models/)
- [Accessibility As A Platform Contract](./accessibility-as-a-platform-contract/)
