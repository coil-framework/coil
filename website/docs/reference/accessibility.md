---
title: Accessibility
---

Accessibility is a runtime and UI contract in Davenda, not only a theme review item.

This page documents the practical expectations for customer apps and module surfaces.

## Platform Expectations

Davenda’s HTML-first model should make these things normal:

- semantic document structure
- forms that work without JavaScript
- clear labels and descriptions
- visible focus states
- keyboard-usable interaction patterns
- accessible fragment updates

The framework cannot enforce perfect output automatically, but it is expected to make accessible implementation easier than inaccessible implementation.

## What Customer Apps Need To Check

Review themes and templates for:

- heading order
- landmark usage
- keyboard reachability
- focus visibility
- color contrast
- motion sensitivity
- status and validation messaging

These are not “nice to have” checks. They directly affect whether the app’s HTML-first promise is credible.

## Accessibility And Fragments

When partial updates occur:

- focus should remain stable or move intentionally
- important changes should be announced appropriately
- controls should remain usable without pointer precision

A fragment-driven UI that breaks keyboard flow is still broken even if the page validated on first render.

## Accessibility And Themes

Themes should be reviewed for:

- contrast across light and dark modes
- visible focus rings
- state communicated by more than color alone
- reduced-motion handling

This is why Davenda documents design tokens and theme behavior alongside accessibility instead of treating them as separate disciplines.

## Common Mistakes

### Relying on color alone

Use multiple cues for state and validation.

### Breaking focus during enhancement

Interactive scripts often regress accessibility faster than templates do.

### Treating admin surfaces as exempt

Back-office UIs still need accessible tables, forms, dialogs, and navigation.

## What To Read Next

- [Accessibility As A Platform Contract](../core-concepts/accessibility-as-a-platform-contract.md)
- [Theme Structure](./theme-structure.md)
- [Gitly Theming, Localization, And Accessibility](../use-cases/gitly/theming-localization-and-accessibility.md)
