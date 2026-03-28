---
title: Customer Rust Vs Third-Party WASM
---

Davenda has two different extension models on purpose:

- linked customer Rust for first-party product logic
- runtime-installed WASM for bounded, lower-trust extensions

If you blur them together, the security and operability story gets weaker quickly.

## What Linked Customer Rust Is

Linked customer Rust is compiled into the customer workspace and shipped with the app binary.

Concrete examples:

- `apps/shoppr/crates/shoppr-backend/src/lib.rs`
- `apps/shoppr/backend/shoppr-loyalty-backend/src/lib.rs`
- `apps/gitly/crates/gitly-backend/src/lib.rs`

That path is for:

- first-party business rules
- customer-owned checkout and webhook policy
- CMS publish hooks
- product-specific data shaping

## What Runtime-Installed WASM Is

Runtime-installed WASM is pinned in the app manifest and loaded through the WASM host boundary at
runtime.

Concrete examples:

- `apps/shoppr/extensions/shoppr-waitlist-tools/package.toml`
- `apps/gitly/extensions/gitly-community-pulse/package.toml`
- `apps/gitly/extensions/gitly-actions-scheduler/package.toml`

That path is for:

- bounded runtime-installed behavior
- marketplace-style or third-party integrations
- explicit slot-based contributions

## How They Are Loaded

### Linked Rust

Linked Rust is loaded by the customer composition root.

Shoppr does it in `apps/shoppr/crates/shoppr-app/src/lib.rs` by creating:

- `vec![Box::new(shoppr_backend::plugin())]`

Gitly does the same in `apps/gitly/crates/gitly-app/src/lib.rs`.

### WASM

WASM packages are:

1. declared in `app.toml`
2. described by `package.toml`
3. compiled or loaded by the customer app bootstrap code
4. installed into explicit extension slots

See:

- `apps/shoppr/crates/shoppr-app/src/extensions.rs`
- `apps/gitly/crates/gitly-app/src/extensions.rs`

## Instance Model And Lifecycle

### Linked Rust lifecycle

Linked Rust is part of the deployed customer binary. If you change it, you rebuild and redeploy
the app.

That means:

- one deployment artifact
- one release process
- one compile-time dependency graph

### WASM lifecycle

WASM is still checked in by the customer in these demos, but the runtime model is installation
driven rather than compile-time linking.

That means:

- package manifest and artifact checksum matter
- the host validates explicit handlers and grants
- the extension runs only through declared slots

## What Each Model Can Access

### Linked Rust can use

- stable customer SDK hook traits
- typed request context
- auth, audit, commerce, jobs, repository, HTTP, and asset facades exposed by the runtime

### WASM can use

- only the host APIs and grants explicitly exposed to the installed package
- only the extension points the package and installation declare

That difference is intentional. Linked Rust is trusted customer code. WASM is a bounded extension
surface.

## Packaging And Distribution

### Linked Rust

Package linked code as part of the customer workspace:

- customer app crate
- customer backend crate
- optional domain library crate

Shoppr shows this clearly in:

- `apps/shoppr/Cargo.toml`
- `apps/shoppr/crates/shoppr-backend/Cargo.toml`
- `apps/shoppr/backend/shoppr-loyalty-backend/Cargo.toml`

### WASM

Package runtime-installed behavior with:

- `package.toml`
- one or more handlers
- a built artifact
- a pinned checksum in `app.toml`

Gitly's checked-in packages are the clearest current examples.

## When To Choose Which

Choose linked Rust when:

- the logic is first-party
- the customer owns the release
- the behavior needs richer facades or tighter typing

Choose WASM when:

- the behavior should stay bounded
- you want explicit host grants
- you want the runtime-installed package model

## Common Mistakes

- Putting first-party business rules into WASM just because it feels “more pluggable.”
- Using linked Rust for behavior that really needs a harder trust boundary.
- Treating WASM as a second unrestricted backend instead of a host-governed extension model.

## Read Next

- [Composition And `davenda-all`](./composition.md)
- [Shoppr Linked Rust Backend](../use-cases/shoppr/linked-rust-backend.md)
- [Shoppr WASM Extensions](../use-cases/shoppr/wasm-extensions.md)
- [Gitly API And Background Work](../use-cases/gitly/api-and-background-work.md)
