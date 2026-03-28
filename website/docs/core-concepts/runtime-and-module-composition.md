---
title: Runtime And Module Composition
---

Davenda is easiest to misuse if you think of composition as "import a few crates and see what happens."

Composition is a first-class part of the runtime model.

## What It Is

Runtime composition is the process that turns:

- the runtime
- the chosen official modules
- the customer app manifest
- customer plugins
- templates and theme assets

into one coherent application plan.

Official modules are not passive libraries. They contribute explicit surfaces such as routes, capabilities, jobs, admin pages, integration points, and data model elements.

## Why It Exists

Frameworks often drift into invisible composition, where startup code, route wiring, permissions, and module boundaries are spread across too many places.

Davenda tries to keep composition explicit because it affects:

- which product batteries are linked
- which route surfaces exist
- which capabilities are bound
- which operational surfaces are available
- which customer hooks can participate

## How It Works

At a high level, the customer binary:

1. chooses the runtime entrypoint
2. registers official modules
3. registers customer plugins
4. loads app/config inputs
5. builds a runtime plan
6. starts the server or operator process

The manifest then decides which parts of the linked battery are actually enabled for that application.

That split matters:

- linking a module makes it available to the application
- enabling it in the customer app makes it part of the product shape

## Why `davenda-all` Exists

`davenda-all` is the convenience battery. It exists so developers can start with a coherent full stack while learning or building the default path.

It is useful when:

- you want the standard official stack
- you are evaluating the platform
- you do not yet need tight dependency control

It is not the only valid entrypoint. Narrower composition is still part of the intended model.

## Common Mistakes

### Confusing linked with enabled

Linking a module into the binary is not the same as enabling it in the customer application.

### Treating module boundaries as presentation-only

Modules affect runtime behavior, auth, data model shape, and operations, not just templates.

### Hiding composition in too many helpers

The customer binary should still make the product shape understandable. If composition becomes hard to trace, debugging the runtime will get harder too.

## What To Read Next

- [Request and render lifecycle](request-and-render-lifecycle.md)
- [Customer apps vs official modules](customer-apps-vs-official-modules.md)
- [Composition and davenda-all](../reference/composition.md)
