---
title: Customer Rust Vs Third-Party WASM
---

Davenda has two different extension models on purpose:

- linked customer Rust for first-party product logic
- runtime-installed WASM for bounded, lower-trust extensions

If you blur them together, the security and operability story gets weaker quickly.

Start with the simplest decision rule:

- if the logic is part of the customer's product and release cycle, use linked Rust
- if the logic should stay runtime-installed and explicitly grant-scoped, use WASM

Concrete contrast:

```rust
// linked Rust: compiled into the customer binary
registry.register_checkout(Box::new(ShopprCheckoutHooks));
```

```toml
# WASM: installed at runtime through app.toml + package.toml
[[extensions]]
id = "shoppr-waitlist-tools"
package_version = "0.1.0"
artifact_sha256 = "..."
customer_app_id = "shoppr"
```

That is the whole distinction:

- linked Rust is part of the app
- WASM is an installed guest package

## The Practical Choice

If you are a customer developer, ask one question first:

> Does this logic ship with my app and evolve with my product policy, or does it need to be a
> separately installed bounded guest?

Use linked Rust if the answer is:

- “this is my product logic”
- “this needs typed facades”
- “this should change in the same release as my app”

Use WASM if the answer is:

- “this should be installable or removable without relinking the app”
- “this should live behind explicit grants”
- “this should stay inside a bounded slot or host API contract”

## What Linked Customer Rust Is

Linked customer Rust is compiled into the customer workspace and shipped with the app binary.

Concrete examples:

That path is for:

- first-party business rules
- customer-owned checkout and webhook policy
- CMS publish hooks
- product-specific data shaping

## What Runtime-Installed WASM Is

Runtime-installed WASM is pinned in the app manifest and loaded through the WASM host boundary at
runtime.

Concrete examples:

That path is for:

- bounded runtime-installed behaviour
- marketplace-style or third-party integrations
- explicit slot-based contributions

## How They Are Loaded

### Linked Rust

Linked Rust is registered by the customer composition root and compiled into the binary.

Minimal flow:

```rust
// customer backend crate
pub fn plugin() -> ShopprBackend {
    ShopprBackend::default()
}

// customer binary
davenda_all::builder()
    .with_customer_plugin(shoppr_backend::plugin())
    .run_from_env()
```

### WASM

WASM is declared in `app.toml`, described by `package.toml`, and installed into explicit extension points at runtime.

Minimal flow:

```toml
[[extensions]]
id = "shoppr-waitlist-tools"
package_version = "0.1.0"
artifact_sha256 = "..."
customer_app_id = "shoppr"

[[extensions.handlers]]
id = "home.waitlist.banner"
grants = []
```

plus:

```toml
[[handlers]]
id = "home.waitlist.banner"
export = "exports.home_waitlist_banner"
point = "render-hook"
target = "cms.page.render"
grants = []
```

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

## Quick Comparison

Linked Rust:

- trusted customer code
- richer typed SDK facades
- compile-time linked
- best for product policy

WASM:

- bounded guest code
- explicit host API and grants
- runtime-installed model
- best for smaller, swappable integrations

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

Package linked code as part of the customer workspace.

That usually means:

- one backend crate
- one customer binary registration point
- normal Cargo dependencies

### WASM

Package runtime-installed behaviour with a package manifest, handlers, built artifact, and a pinned checksum in `app.toml`.

That usually means:

- one `extensions/<id>/package.toml`
- one `.wasm` artifact
- one `[[extensions]]` install block in `app.toml`
- one or more approved handler grants

## When To Choose Which

Choose linked Rust when:

- the logic is first-party
- the customer owns the release
- the behaviour needs richer facades or tighter typing

Choose WASM when:

- the behaviour should stay bounded
- you want explicit host grants
- you want the runtime-installed package model

## A Good Heuristic

If the behaviour needs:

- customer-specific checkout policy
- customer-specific webhook logic
- customer-specific CMS publish rules

use linked Rust.

If the behaviour needs:

- a render slot
- a narrow API handler
- a small scheduled job
- an explicit install/uninstall model

use WASM.

## Common Mistakes

- Putting first-party business rules into WASM just because it feels “more pluggable.”
- Using linked Rust for behaviour that really needs a harder trust boundary.
- Treating WASM as a second unrestricted backend instead of a host-governed extension model.

## Full Implementation

Linked Rust examples:

- `apps/shoppr/crates/shoppr-backend/src/lib.rs`
- `apps/shoppr/backend/shoppr-loyalty-backend/src/lib.rs`
- `apps/gitly/crates/gitly-backend/src/lib.rs`
- `apps/shoppr/crates/shoppr-app/src/lib.rs`
- `apps/gitly/crates/gitly-app/src/lib.rs`

WASM examples:

- `apps/shoppr/extensions/shoppr-waitlist-tools/package.toml`
- `apps/gitly/extensions/gitly-community-pulse/package.toml`
- `apps/gitly/extensions/gitly-actions-scheduler/package.toml`
- `apps/shoppr/crates/shoppr-app/src/extensions.rs`
- `apps/gitly/crates/gitly-app/src/extensions.rs`

Workspace packaging examples:

- `apps/shoppr/Cargo.toml`
- `apps/shoppr/crates/shoppr-backend/Cargo.toml`
- `apps/shoppr/backend/shoppr-loyalty-backend/Cargo.toml`

## Read Next

- [Composition And `davenda-all`](./composition.md)
- [Shoppr Linked Rust Backend](../use-cases/shoppr/linked-rust-backend.md)
- [Shoppr WASM Extensions](../use-cases/shoppr/wasm-extensions.md)
- [Gitly API And Background Work](../use-cases/gitly/api-and-background-work.md)
