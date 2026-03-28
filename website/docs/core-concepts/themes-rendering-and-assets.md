---
title: Themes, Rendering, And Assets
---

Davenda themes are the customer-owned rendering layer of the product.

They are not just a CSS bundle. A theme shapes how layouts, fragments, assets, and frontend enhancements come together across the whole application.

## What A Theme Is

A theme usually combines four things:

- **templates and layouts** that define the HTML structure
- **design tokens** that give names to colors, spacing, typography, motion, and related visual primitives
- **assets** such as CSS, JavaScript, images, and icons
- **presentation conventions** for light, dark, or system-driven variants

In the checked-in demos, the theme lives under `theme/` and works together with the customer template tree under `templates/`.

## Why Davenda Treats Themes As Part Of The Platform Contract

Themes matter to more than appearance.

They directly affect:

- accessibility
- SEO and metadata rendering
- perceived performance
- brand consistency across modules
- how easily official module surfaces can be restyled without being forked

That is why Davenda documents them as a framework concept rather than as an implementation detail.

## The Resolution Model

Davenda does not let every module invent its own skinning mechanism.

Instead:

- core owns the rules for discovering templates and assets
- official modules render semantic surfaces and slots
- the customer app decides the actual shell, layouts, design tokens, and published assets

This is what allows a customer app to keep CMS, commerce, memberships, and admin visually coherent without copying those modules wholesale.

## Theme Assets And Hashed Publication

Customer templates should reference logical assets through the platform helpers rather than hard-coding public URLs or unhashed file names.

The runtime publishes hashed outputs and tracks them through an asset manifest. That gives Davenda:

- deterministic cache busting
- safer release switching
- environment-independent template references

The practical rule is simple: templates should ask for an asset by logical name, and the runtime should resolve the current published artifact.

## Light, Dark, And System Mode

Davenda does not force one built-in theming toggle model, but it does expect the theme system to be able to support:

- a stable default visual mode
- explicit light/dark variants where desired
- system-mode alignment when the product chooses to support it

The checked-in demos model this through customer-owned CSS variables and a small frontend script rather than through a framework-wide magic theme flag.

That is the intended boundary:

- Davenda gives you the runtime and asset model
- the customer theme chooses how light, dark, and system behavior should work in the product

## Design Tokens

Design tokens are not just a naming preference. They are the stable surface that lets customer apps rebrand official module output without depending on DOM trivia.

Well-used tokens describe semantics such as:

- background and surface roles
- text and muted text roles
- accent and action colors
- spacing scales
- focus rings
- motion preferences

This makes a large app easier to maintain than scattering hard-coded values through every page.

## Common Mistakes

### Treating the theme as only CSS

The theme also includes structural templates, token decisions, and the asset publication model.

### Hard-coding public asset URLs

That bypasses the asset manifest and usually breaks cache busting or local/production parity.

### Rebuilding module UI instead of restyling it

If a theme change requires duplicating entire module screens, the product is fighting the module boundary.

## What To Read Next

- [Template Language Reference](../reference/template-language.md)
- [Theme Structure Reference](../reference/theme-structure.md)
- [Accessibility Reference](../reference/accessibility.md)
