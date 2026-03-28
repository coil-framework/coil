---
title: Accessibility
---

This page documents the practical accessibility expectations for Davenda customer apps.

## What Is This?

Davenda treats accessibility as part of the rendering and interaction contract for:

- public pages
- account flows
- storefront and checkout surfaces
- admin pages
- progressively enhanced fragments

This page focuses on what you need to do in templates and themes today.

## Why Does It Exist?

Davenda is HTML-first. That means the framework and the customer app have unusually strong leverage
over:

- document landmarks
- form semantics
- focus behavior
- keyboard access
- route and navigation structure
- locale metadata

If those are wrong, the product is wrong even if it “looks fine.”

## When Should I Use This Guidance?

Use this page whenever you are building or reviewing:

- a new page shell
- a form
- a navigation header
- an admin table
- a fragment update flow
- theme-level motion or contrast changes

## What Davenda Helps With Today

Davenda gives you a few concrete advantages:

- server-rendered HTML-first pages by default
- request-level locale values so `<html lang>` can be correct
- module and customer templates that remain ordinary HTML
- progressive enhancement on top of a real baseline page instead of a client-only shell

What Davenda does **not** do today:

- it does not run a full automatic accessibility validator over every template
- it does not guarantee customer themes keep sufficient contrast or focus treatment
- it does not replace disciplined markup review

That boundary needs to be explicit.

## What Exact Files Should You Look At?

Canonical checked-in examples:

- public shell and navigation:
  - `apps/gitly/templates/gitly/home.html`
  - `apps/gitly/templates/gitly/explore.html`
- search and language controls:
  - `apps/gitly/templates/gitly/search.html`
- storefront forms:
  - `apps/shoppr/templates/commerce/cart.html`
  - `apps/shoppr/templates/commerce/checkout.html`
- account surfaces:
  - `apps/shoppr/templates/pages/account.html`
  - `apps/shoppr/templates/memberships/account.html`
- admin examples:
  - `apps/shoppr/templates/admin/dashboard.html`
  - `apps/shoppr/templates/commerce/catalog-admin.html`

## Practical Markup Examples

### Skip link and landmarks

Gitly demonstrates the correct baseline pattern:

```html
<a class="skip-link" href="#main">Skip to content</a>
<nav aria-label="Primary navigation">...</nav>
<main id="main">...</main>
```

Why it matters:

- keyboard users can bypass repeated navigation
- screen readers get explicit landmarks

### Search form with real labels

Gitly uses a visually hidden label instead of relying only on placeholder text:

```html
<form method="get" action="/search">
  <span class="sr-only">Search</span>
  <input type="search" name="q" />
  <button type="submit">Search</button>
</form>
```

### Status announcement

Gitly’s API fallback card is a good pattern for live status:

```html
<aside role="status" aria-live="polite">API hydration failed.</aside>
```

Use this for user-visible asynchronous status changes, not just visual banners.

### Tables and real headings

Gitly’s issues and pulls pages demonstrate a correct baseline:

- real `<table>`
- `<caption>`
- `<th scope="col">`

Use tables for tabular data, not CSS layout.

## What Remains The App’s Responsibility?

Customer apps still own:

- actual heading hierarchy
- visible focus styles
- contrast in every supported theme mode
- error message wording and placement
- screen-reader labels for customer-owned controls
- accessible dialog, drawer, or widget behavior introduced by customer JS

Davenda gives you the contract and the right rendering shape. You still need to use it correctly.

## Accessibility And Progressive Enhancement

If a page becomes better with JavaScript, that is fine. If it only becomes usable with JavaScript,
that is a problem.

For fragment and enhancement flows:

- forms should stay forms
- links should stay links
- focus should remain stable or move intentionally
- significant changes should expose status messaging

A fragment update that breaks keyboard flow is still an accessibility bug.

## Theme-Level Accessibility

Review `theme/assets/site.css` and `theme/assets/site.js` for:

- visible focus rings
- contrast across light, dark, and system mode
- reduced-motion handling where animation exists
- state communicated by more than color alone
- reasonable target size for interactive elements

Gitly is the best checked-in example for:

- light, dark, and system mode controls
- language switcher semantics
- skip link and landmark usage

## Common Mistakes

### Using placeholder text as the only label

Search, checkout, and admin forms should still expose a real label or screen-reader-only label.

### Hiding focus styles in the theme

This is one of the fastest ways to make a product unusable by keyboard.

### Treating admin as exempt

Internal tools still need accessible tables, forms, and navigation.

### Replacing semantic HTML with div-only interaction wrappers

Davenda’s HTML-first model is an advantage. Do not throw it away.

## What Should I Read Next?

- [Template Language](./template-language.md)
- [Theme Structure](./theme-structure.md)
- [Accessibility As A Platform Contract](../core-concepts/accessibility-as-a-platform-contract.md)
- `apps/gitly/templates/gitly/`
- `apps/shoppr/templates/commerce/`
