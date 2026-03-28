---
title: Theme Structure
---

Davenda themes live in the customer app and provide the visual and frontend interaction layer for the product.

This page explains the expected shape, the common files, and how light/dark/system behavior should fit into that structure.

## Typical Theme Layout

The checked-in demos use a theme layout like this:

```text
theme/
  tokens.toml
  assets/
    site.css
    site.js
    logo.svg
```

The customer app also has a template tree under `templates/`. Together, `templates/` and `theme/` make up the customer-owned UI layer.

## What Each File Does

### `tokens.toml`

This is the design-token layer.

Use it for stable semantic values such as:

- color roles
- spacing scales
- type scales
- radii
- elevation
- motion preferences

The exact token vocabulary can evolve, but the purpose stays the same: semantic design values should not be scattered as raw literals through every stylesheet and template.

### `assets/site.css`

This is the main compiled or authored stylesheet for the customer app.

It normally owns:

- page layouts
- component styling
- responsive behavior
- focus states
- dark/light variants
- utility or component-specific styling that the templates depend on

### `assets/site.js`

This is the progressive enhancement layer.

It should be used for:

- carousels
- accordions
- theme switching
- locale switching support
- fragment or API hydration that improves, but does not replace, the baseline page behavior

It should not become a second application runtime that recreates the whole server-rendered page model.

### Images And Icons

Files such as `logo.svg` or static imagery belong under `assets/` and should be published through the asset pipeline like the rest of the theme assets.

## Where Layouts And Templates Live

Davenda keeps layouts and page templates under `templates/`, not under `theme/`.

That split is useful because:

- `templates/` defines structure and binding
- `theme/` defines visual tokens and frontend assets

They work together, but they are not the same thing.

## Dark, Light, And System Mode

Davenda does not force one universal theme-mode implementation. The customer theme chooses the product behavior.

A sound approach is:

- server-render a sensible default mode
- use design tokens or CSS variables for light and dark values
- let `site.js` manage explicit light/dark/system preference when the product wants that behavior
- respect reduced-motion and accessibility concerns while switching modes

The demos model this with `data-theme` driven CSS rather than a framework-global mode flag.

## Asset Publication Rules

Theme assets are published as hashed artifacts. Templates should reference them through the asset helper or the runtime’s asset manifest model rather than hard-coded filenames.

This keeps:

- local development
- staging
- production

aligned under one asset contract.

## Common Mistakes

### Putting layout logic into CSS alone

Templates still need to own the semantic HTML structure.

### Treating `site.js` like a SPA shell

Enhancement scripts should improve the HTML-first path, not replace it.

### Ignoring theme governance

Theme changes can break accessibility, SEO, or operability just as easily as backend changes if they are not reviewed with the same discipline.

## What To Read Next

- [Template Language](./template-language.md)
- [Accessibility Reference](./accessibility.md)
- [SEO Reference](./seo.md)
