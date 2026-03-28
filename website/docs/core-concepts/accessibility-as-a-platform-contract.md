---
title: Accessibility As A Platform Contract
---

Davenda treats accessibility as part of the platform contract.

That is not a slogan. It means accessibility is expected to shape the runtime, rendering, and interaction model rather than appearing only as a design review at the end.

## Why It Lives At The Platform Level

Davenda controls enough of the application model that it can materially help or harm accessibility.

The framework influences:

- document structure
- form rendering and validation
- route and navigation patterns
- fragment updates
- focus movement
- keyboard interaction
- the semantics of admin and product surfaces

If those primitives are careless, customer apps inherit unnecessary accessibility debt immediately.

## What The Platform Should Make Easier

Davenda’s HTML-first model should make the accessible path the natural path for:

- semantic headings and landmarks
- working forms without JavaScript
- accessible validation summaries
- keyboard-usable navigation
- visible focus states
- meaningful live updates for fragment-driven interactions

The framework cannot guarantee every customer theme is accessible, but it can avoid making accessible implementation unnatural.

## Where Responsibility Still Sits With The Customer App

Customer apps still own:

- actual heading structure and page semantics
- contrast decisions in the theme
- labels and descriptions for customer-specific components
- whether motion, theme, and interaction choices remain accessible

Davenda gives the constraints and primitives. The customer app still has to use them well.

## Accessibility And Progressive Enhancement

Fragment updates and enhanced UI are where many apps regress.

Davenda’s model expects enhanced UI to:

- preserve working server-driven fallbacks
- keep focus stable or move it intentionally
- announce important updates where appropriate
- avoid requiring pointer-only interaction

That is why the docs treat accessibility and progressive enhancement as linked topics.

## What To Read Next

- [Request And Render Lifecycle](./request-and-render-lifecycle.md)
- [Accessibility Reference](../reference/accessibility.md)
- [Theme Structure Reference](../reference/theme-structure.md)
