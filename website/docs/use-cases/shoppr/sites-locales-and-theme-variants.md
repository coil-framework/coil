---
title: Sites, Locales, And Theme Variants
---

Shoppr is the main demonstration of Davenda’s multi-site model because it shows one customer app serving multiple storefront sites with different regional emphasis while sharing one runtime and one codebase.

## The Three Sites

Shoppr currently declares three sites:

- `shoppr-uk`
- `shoppr-fr`
- `shoppr-pl`

Those are configured in `app.toml` for the product contract and mirrored in `platform.dev.toml` for runtime host resolution.

## Why Shoppr Uses Sites Instead Of Only Locales

Shoppr uses sites because the differences are broader than translation alone:

- different hosts
- different default locales
- different branding emphasis
- different catalog emphasis and availability

If Shoppr used locale alone, the product would lose a clear place to express those differences.

## Locale Within Each Site

Each site still supports multiple locales. That is the important nuance:

- site chooses the public regional boundary
- locale chooses the language/formatting layer within that boundary

This is the model you should copy for serious multi-region commerce.

## Theme Variants

The theme can also participate in the site model without cloning the whole app.

In practice, that means:

- shared layouts and assets
- site-aware branding values
- site-aware hero copy and editorial framing
- possibly different navigation emphasis or campaign treatment

The goal is one customer app with controlled variation, not three near-duplicate apps.

## What To Read Next

- [Sites, Locales, And Markets](../../core-concepts/sites-locales-and-markets.md)
- [Internationalization Reference](../../reference/internationalization.md)
- [Theme Structure Reference](../../reference/theme-structure.md)
