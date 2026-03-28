---
title: Theming, Localization, And Accessibility
---

Gitly is the best non-commerce example of Davenda's theme and frontend behavior model.

## Theme Files To Read

Start with:

- `apps/gitly/theme/assets/site.css`
- `apps/gitly/theme/assets/site.js`
- `apps/gitly/theme/tokens.toml`

These files show how the customer app owns visual identity and browser behavior directly.

## Theme Switching

Gitly's theme switcher lives in `apps/gitly/theme/assets/site.js`.

That file owns:

- `light`, `dark`, and `system` mode switching
- persistence of the chosen theme
- the small client-side behavior needed by the product shell

This is a good example because it keeps theming in customer-owned frontend assets rather than a
framework-owned control panel.

## Localization In Practice

Gitly's localization story is split across:

- `apps/gitly/app.toml`
- `apps/gitly/platform.dev.toml`
- `apps/gitly/theme/assets/site.js`

The app and platform config declare supported locales and localized routes. Then `site.js` carries
the customer-owned translation tables and route-aware locale switching behavior.

That is a useful example because it shows Davenda's i18n model in a product that is not a store.

## Accessibility In A Dense Product UI

Gitly is also a strong accessibility example because its pages are not simple marketing layouts.

Read:

- `apps/gitly/templates/gitly/repository.html`
- `apps/gitly/templates/gitly/issues.html`
- `apps/gitly/templates/gitly/pulls.html`
- `apps/gitly/theme/assets/site.css`

These files demonstrate:

- visible focus states
- keyboard-oriented navigation patterns
- readable dense tables and panels
- theme and contrast concerns

That makes Gitly a useful reference app for product UIs that need more than hero banners and cards.

## Adapt This For Your App

Copy these patterns:

- keep theme assets customer-owned
- let the app own translated UI copy where that is the current product choice
- make theme switching a visible product behavior
- keep accessibility work in the same frontend layer as the actual UI

## Read Next

- [Theme Structure](../../reference/theme-structure.md)
- [Internationalization](../../reference/internationalization.md)
- [Accessibility](../../reference/accessibility.md)
