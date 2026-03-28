---
title: Theming, Localization, And Accessibility
---

Gitly is useful because it shows that Davenda’s theme, locale, and accessibility model is not tied to storefront pages.

## Theme Switching

Gitly’s `theme/assets/site.css` and `theme/assets/site.js` model a product shell with explicit light, dark, and system-aware behavior.

This is a good example because the switching behavior belongs to the customer app, not to a hidden global theme service.

## Localization

Gitly supports localized routes and customer-owned translation dictionaries for parts of the frontend experience. That makes it a strong counterexample to the idea that Davenda’s i18n model only matters to ecommerce.

## Accessibility

Gitly’s repository and dashboard-style pages are also useful for accessibility because they include:

- navigation-heavy layouts
- table-like information presentation
- theme and contrast concerns
- interactive controls that still need keyboard-friendly behavior

## What To Read Next

- [Theme Structure](../../reference/theme-structure.md)
- [Internationalization Reference](../../reference/internationalization.md)
- [Accessibility Reference](../../reference/accessibility.md)
