---
title: Accessibility As A Platform Contract
---

This page explains what “accessibility as a platform contract” means in practical Davenda terms.

## What Is This?

Davenda treats accessibility as a shared contract across:

- runtime rendering
- official module surfaces
- customer templates
- customer theme assets
- progressively enhanced interactions

This is broader than “check contrast before launch.”

## Why Does It Live At The Platform Level?

Davenda controls enough of the application model that it can either help or harm accessibility very
quickly.

The framework influences:

- document structure
- locale metadata
- form rendering
- navigation shells
- fragment updates
- first-render HTML

So accessibility cannot be left entirely to late theme review.

## When Should Developers Think About It?

From the first template.

Do not wait until the visual design is “done.” In Davenda, the semantics are already visible early:

- page shells
- nav bars
- forms
- tables
- status messaging
- account and checkout flows

## What Davenda Validates Today

This is the honest answer:

Davenda does **not** currently run a full automatic accessibility validator over customer templates.

What the platform does give you today is:

- server-rendered HTML-first output
- request-driven locale values so `lang` can be correct
- ordinary semantic HTML templates instead of opaque view bytecode
- module and customer surfaces that can be reviewed directly in checked-in HTML

What remains app responsibility:

- heading order
- contrast
- focus states
- live-region usage
- keyboard handling in customer JS

## Practical Markup Examples

### Navigation and skip links

Gitly demonstrates the right baseline shape in files such as:

- `apps/gitly/templates/gitly/home.html`
- `apps/gitly/templates/gitly/explore.html`

Pattern:

```html
<a class="skip-link" href="#main">Skip to content</a>
<nav aria-label="Primary navigation">...</nav>
<main id="main">...</main>
```

Why this matters:

- repeated navigation becomes skippable
- landmarks are explicit
- screen-reader orientation improves immediately

### Forms

Use real form controls and labels. Gitly’s search forms and Shoppr’s cart and checkout templates are
the best checked-in patterns:

- `apps/gitly/templates/gitly/search.html`
- `apps/shoppr/templates/commerce/cart.html`
- `apps/shoppr/templates/commerce/checkout.html`

Baseline rule:

- forms must remain usable without JavaScript

### Tables

Gitly’s issues and pulls pages show the correct baseline:

- real `<table>`
- real `<caption>`
- real `<th scope="col">`

See:

- `apps/gitly/templates/gitly/issues.html`
- `apps/gitly/templates/gitly/pulls.html`

### Status and live feedback

Gitly’s API fallback card shows the minimal correct live-status shape:

```html
<aside role="status" aria-live="polite">...</aside>
```

See:

- `apps/gitly/templates/gitly/home.html`

## Accessibility And Progressive Enhancement

Progressive enhancement is where many otherwise good server-rendered apps regress.

In Davenda, a good enhancement path means:

- the base page already works
- focus remains stable or moves intentionally
- important changes expose status feedback
- pointer-only interaction is not required

This matters especially for:

- cart updates
- checkout progress
- account or admin panels
- search and language controls

## Accessibility And Themes

The theme can easily undo good semantics.

Customer theme review should cover:

- visible focus rings
- contrast in light, dark, and system mode
- reduced-motion handling
- state not conveyed only by color

Gitly is the canonical checked-in example for reviewing these concerns because it ships:

- language switcher
- theme switcher
- skip link
- screen-reader-only labels

across multiple pages under `apps/gitly/templates/gitly/`.

## What Remains The App’s Responsibility?

Customer apps still own:

- semantic page structure
- customer-specific labels and descriptions
- accessible dialogs and drawers if they add them
- JS interaction patterns
- theme-level visual accessibility

Davenda makes the right path possible and normal. It does not absolve the customer app from using
that path well.

## Constraints And Common Mistakes

### Treating admin as exempt

Internal tools still need accessible forms, tables, and navigation.

### Removing focus styles in the design pass

That instantly breaks keyboard usability.

### Using placeholders as labels

Search and checkout controls still need real labels.

### Assuming first render passing means fragment updates are also accessible

Partial updates need their own review.

## What Should I Read Next?

- [Accessibility](../reference/accessibility.md)
- [Themes, Rendering, And Assets](./themes-rendering-and-assets.md)
- [Request And Render Lifecycle](./request-and-render-lifecycle.md)
- `apps/gitly/templates/gitly/`
- `apps/shoppr/templates/commerce/`
