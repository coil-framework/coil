---
title: Gitly Overview
---

Gitly is a supporting example for a broader point: Coil is a customer-app platform, not a
storefront-only framework.

Use this page when you want to answer:

- what a non-commerce Coil app looks like
- which platform patterns survive when you remove catalog and checkout concerns
- how to read the Gitly demo without treating it as the only valid app shape

## The Core Pattern

Coil customer apps keep the same structure regardless of product type:

- `app.toml` declares the customer product contract
- platform config wires runtime services and secrets
- the customer app crate composes routes, modules, linked plugins, and extensions
- the customer binary owns validate, migrate, assets, and serve/up

Gitly matters because it proves that structure still works for a forge-style app.

## Canonical Customer-App Contract

The smallest useful thing to look at is the app manifest shape. Gitly’s `apps/gitly/app.toml`
contains the canonical ingredients:

```toml
[app]
name = "gitly"
display_name = "Gitly"

[modules]
enabled = ["admin", "cms", "media", "gitly-showcase"]
```

That snippet shows the real lesson:

- customer apps choose a product identity
- they enable only the modules they need
- they can add customer-owned product modules such as `gitly-showcase`

This is a better first takeaway than "Gitly has repository pages."

## Canonical Customer Binary Shape

The lifecycle surface for a non-commerce app should look the same as it does for a store.

Gitly’s binary in `apps/gitly/crates/gitly-bin/src/main.rs` exposes:

```rust
enum Command {
    Describe,
    Validate,
    Assets { command: AssetsCommand },
    Migrate { command: MigrateCommand },
    Serve { bind: Option<String> },
    Up { bind: Option<String> },
    ExtensionChecksums,
    LinkedBackend { command: LinkedBackendCommand },
}
```

That is the real platform story:

- customer apps own their lifecycle
- the same verbs work for commerce and non-commerce products
- linked backends and runtime-installed extensions remain first-class

## What Gitly Adds On Top

Gitly uses that platform shape to demonstrate:

- customer-owned route vocabulary
- linked Rust data shaping
- API-style endpoints
- theme switching
- localized UI copy
- scheduled-task demos
- runtime-installed WASM

The product is different. The platform contract is the same.

## Gitly As The Supporting Example

### Product contract and runtime config

Read:

- `apps/gitly/app.toml`
- `apps/gitly/platform.dev.toml`

These show:

- one-site non-commerce routing
- locales and localised routes
- a narrow module set
- jobs, storage, cache, and asset config that look like any other Coil app

### Composition root

Read:

- `apps/gitly/crates/gitly-app/src/lib.rs`

This file demonstrates:

- customer-owned route mounting
- customer module registration
- linked plugin registration
- extension loading

### Product templates

Read:

- `apps/gitly/templates/gitly/home.html`
- `apps/gitly/templates/gitly/explore.html`
- `apps/gitly/templates/gitly/repository.html`
- `apps/gitly/templates/gitly/issues.html`
- `apps/gitly/templates/gitly/pulls.html`
- `apps/gitly/templates/gitly/actions.html`
- `apps/gitly/templates/gitly/organization.html`
- `apps/gitly/templates/gitly/profile.html`
- `apps/gitly/templates/gitly/search.html`

These are supporting evidence that Coil’s HTML-first model works for dense product shells too.

### Linked Rust and WASM

Read:

- `apps/gitly/crates/gitly-backend/src/lib.rs`
- `apps/gitly/extensions/gitly-community-pulse/package.toml`
- `apps/gitly/extensions/gitly-actions-scheduler/package.toml`
- `apps/gitly/crates/gitly-app/src/extensions.rs`

That set shows the same first-party-vs-bounded-extension split that Shoppr shows, but without
commerce vocabulary dominating the example.

## Practical Rules To Copy

- start from the customer-app contract, not from page mocks
- keep the customer binary as the first operational surface
- use customer-owned modules and routes to define product vocabulary
- use linked Rust for first-party data and policy
- use WASM for bounded installable behaviour

## Full Implementation Pointers

- `apps/gitly/app.toml`
- `apps/gitly/platform.dev.toml`
- `apps/gitly/crates/gitly-app/src/lib.rs`
- `apps/gitly/crates/gitly-bin/src/main.rs`
- `apps/gitly/crates/gitly-backend/src/lib.rs`
- `apps/gitly/templates/gitly/`
- `apps/gitly/extensions/gitly-community-pulse/package.toml`
- `apps/gitly/extensions/gitly-actions-scheduler/package.toml`

## Read Next

- [Product Structure](./product-structure.md)
- [API And Background Work](./api-and-background-work.md)
- [Build And Deploy](./build-and-deploy.md)
