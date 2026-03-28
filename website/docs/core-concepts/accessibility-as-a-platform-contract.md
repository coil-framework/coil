---
title: Accessibility As A Platform Contract
---

In Coil, accessibility is part of the rendering contract, not a late design cleanup pass.

## Start With The Practical Standard

A good Coil page should already make sense before any enhancement script runs.

A minimal public shell should look like this:

```html
<a class="skip-link" href="#main">Skip to content</a>
<nav aria-label="Primary navigation">...</nav>
<main id="main">...</main>
```

That tiny snippet carries most of the point:

- navigation is a real landmark
- repeated page furniture is skippable
- the primary content target is explicit

Accessibility is therefore visible in the template itself, not only in a testing report.

## Why Is This A Platform Concern?

Coil influences:

- document structure
- locale metadata
- forms
- navigation shells
- fragment updates
- the baseline HTML that ships before JavaScript enhancement

Because the framework controls so much of that path, it has a real responsibility to make the
accessible path the normal path.

## What Coil Helps With Today

Current practical advantages:

- HTML-first rendering
- request-driven locale values so `<html lang>` can be correct
- module and customer surfaces expressed as reviewable HTML
- progressive enhancement layered on top instead of replacing the baseline page

Current honest limitation:

- Coil does not currently run a full automatic accessibility validator over every customer
  template

So the platform helps strongly, but the customer app still has to execute well.

## Forms, Tables, And Status Messages

### Forms

A Coil form should remain a real form:

```html
<form method="get" action="/search">
  <span class="sr-only">Search</span>
  <input type="search" name="q" />
  <button type="submit">Search</button>
</form>
```

What this teaches:

- placeholder text is not the only label
- the control works before JS
- semantics are visible in the template

### Tables

A real data table should still be a real table:

```html
<table>
  <caption>Open issues</caption>
  <thead>
    <tr>
      <th scope="col">Issue</th>
      <th scope="col">Owner</th>
    </tr>
  </thead>
</table>
```

### Status feedback

A meaningful update should expose status semantics:

```html
<aside role="status" aria-live="polite">API hydration failed.</aside>
```

This matters especially for fragment or enhancement flows.

## Progressive Enhancement Is Part Of Accessibility

An HTML-first product can still regress badly after enhancement.

For enhanced flows, the standard is:

- the base page already works
- focus remains stable or moves intentionally
- important updates expose status feedback
- interaction does not become pointer-only

This is why cart updates, checkout progress, and account panels still need accessibility review even
if the initial page render looks correct.

## What Still Belongs To The Customer App?

Coil does not remove customer responsibility for:

- heading hierarchy
- contrast
- visible focus treatment
- label quality
- theme-level accessibility
- custom JS interactions

A customer app can still break accessibility through a bad theme or bad enhancement choices. The
framework’s job is to make that a deliberate mistake, not the default path.

## Common Mistakes

### Treating admin as exempt

Internal tools still need accessible forms, tables, and navigation.

### Removing focus styles in the theme

That breaks keyboard usability immediately.

### Using placeholders as labels

Search, checkout, and admin controls still need real labeling.

### Assuming fragment updates are accessible because the first render is

Partial updates need their own review.

## Supporting Implementation And Repo Examples

Concrete supporting files:

- `apps/gitly/templates/gitly/home.html`
- `apps/gitly/templates/gitly/explore.html`
- `apps/gitly/templates/gitly/search.html`
- `apps/gitly/templates/gitly/issues.html`
- `apps/gitly/templates/gitly/pulls.html`
- `apps/shoppr/templates/commerce/cart.html`
- `apps/shoppr/templates/commerce/checkout.html`

## What Should I Read Next?

- [Accessibility](../reference/accessibility.md)
- [Themes, Rendering, And Assets](./themes-rendering-and-assets.md)
- [Template Language](../reference/template-language.md)
