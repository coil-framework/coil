---
title: Runtime And Module Composition
---

Coil is easiest to misuse if you think of composition as "import a few crates and see what happens."

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

Coil tries to keep composition explicit because it affects:

- which product batteries are linked
- which route surfaces exist
- which capabilities are bound
- which operational surfaces are available
- which customer hooks can participate

## The Practical Rule

Coil composition has two layers:

- Cargo dependencies decide what the binary can register
- the customer app manifest decides what the application actually enables

That means:

- linking a module makes it available
- enabling it in `app.toml` makes it active for the product

Those are intentionally separate decisions.

## Shoppr As The Concrete Example

Shoppr shows the full composition story in one place:

- `apps/shoppr/Cargo.toml`
- `apps/shoppr/crates/shoppr-bin/src/main.rs`
- `apps/shoppr/crates/shoppr-app/src/lib.rs`
- `apps/shoppr/app.toml`

If you want to see what “real Coil composition” looks like in code, those files are the right starting point.

## How It Works

At a high level, the customer binary:

1. chooses the runtime entrypoint
2. registers official modules
3. registers customer plugins
4. loads app and config inputs
5. builds a runtime plan
6. starts the server or operator process

The manifest then decides which parts of the linked battery are actually enabled for that application.

That split matters:

- linking a module makes it available to the application
- enabling it in the customer app makes it part of the product shape

## Minimal Composition Example

The convenience path is to use `coil` in the customer workspace:

```toml
[workspace.dependencies]
coil = "0.1.0"
```

Then the customer app crate can rely on the official module battery and let the manifest choose which modules are active.

In Shoppr, the runtime bootstrap uses `official_modules_from_config` through the Shoppr app crate in `apps/shoppr/crates/shoppr-app/src/lib.rs`.

That path is a good default when:

- you want the full official stack available
- you are starting quickly
- you do not yet need tight dependency minimisation

## Narrow Composition Example

The convenience battery is not the only model. A customer workspace can choose to depend on narrower crates directly:

```toml
[workspace.dependencies]
coil-runtime = "0.1.0"
coil-cms = "0.1.0"
coil-commerce = "0.1.0"
coil-admin = "0.1.0"
coil-customer-sdk = "0.1.0"
```

Then the customer binary or bootstrap layer explicitly registers only those modules.

This path makes sense when:

- you want strict control over the official battery
- you are building a narrower product
- you want dependency visibility to stay very tight

## Linked Versus Enabled

This is the most important practical distinction in Coil composition.

### Linked

A module is linked when the customer binary has the code available at build time.

### Enabled

A module is enabled when the customer app manifest includes it under:

- [modules.enabled in `app.toml`](../reference/app-toml.md)

### What Happens If They Drift

If the manifest enables a module the binary did not link, the runtime build should fail rather than half-starting an incoherent system.

That is a feature, not a nuisance. It prevents "looks configured, fails at runtime" behaviour.

## What The Customer Plugin Layer Adds

Module composition is not the whole story. The runtime plan is also shaped by linked customer plugins.

In Shoppr, the bootstrap layer registers:

- linked customer Rust hooks
- runtime-installed WASM extension packages

That happens in `apps/shoppr/crates/shoppr-app/src/lib.rs`.

So a full runtime plan is composed from:

- official modules
- app manifest
- auth package
- templates
- linked customer plugins
- installed runtime extensions

## Why `coil` Exists

`coil` is the convenience battery. It exists so developers can start with a coherent full stack while learning or building the default path.

It is useful when:

- you want the standard official stack
- you are evaluating the platform
- you do not yet need tight dependency control

It is not the only valid entrypoint. Narrower composition is still part of the intended model.

## A Good Composition Checklist

Before calling a Coil app “properly composed”, you should be able to answer all of these:

- Which official modules are linked into the binary?
- Which of those modules are enabled in `app.toml`?
- Which auth package is selected?
- Which linked customer plugins are registered?
- Which runtime-installed extensions are declared?
- Which templates and asset roots are active?

If you cannot answer those quickly, the composition story is too hidden.

## Common Mistakes

### Confusing linked with enabled

Linking a module into the binary is not the same as enabling it in the customer application.

### Treating module boundaries as presentation-only

Modules affect runtime behaviour, auth, data model shape, and operations, not just templates.

### Hiding composition in too many helpers

The customer binary should still make the product shape understandable. If composition becomes hard to trace, debugging the runtime will get harder too.

### Treating `coil` as mandatory

It is a convenience battery, not the only valid composition story.

## Read Next

- [Request and render lifecycle](request-and-render-lifecycle.md)
- [Customer apps vs official modules](customer-apps-vs-official-modules.md)
- [Composition and coil](../reference/composition.md)
- [Official modules](../reference/modules.md)
