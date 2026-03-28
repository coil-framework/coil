---
title: WASM Extensions
---

Shoppr demonstrates Davenda's runtime-installed WASM path with a deliberately bounded example.

Use this page to understand what a real checked-in package looks like and how it differs from the
linked backend.

## The Example Package

The concrete package lives here:

- `apps/shoppr/extensions/shoppr-waitlist-tools/package.toml`
- `apps/shoppr/extensions/shoppr-waitlist-tools/README.md`
- `apps/shoppr/extensions/shoppr-waitlist-tools/shoppr-waitlist-tools.wat`

The app manifest pins it here:

- `apps/shoppr/app.toml`

And the customer app compiles and loads it here:

- `apps/shoppr/crates/shoppr-app/src/extensions.rs`

## What The Example Teaches

The Shoppr package is intentionally narrow. It exists to teach:

- package metadata
- artifact checksum pinning
- explicit installed handlers
- runtime-installed compilation and loading
- a bounded render-hook contribution

That restraint is useful. It keeps the extension model honest.

## How The Installation Flow Works

Shoppr's `app.toml` declares the installed extension id, package version, checksum, and handlers.

Then `apps/shoppr/crates/shoppr-app/src/extensions.rs` does the practical work:

- reads the extension install document
- loads `package.toml`
- compiles the checked-in WAT source
- builds the extension manifest
- injects the package into the customer runtime plan

This is the exact flow to study if you want to understand runtime-installed extensions in a real
customer app.

## What Shoppr Uses WASM For

Shoppr uses WASM for a bounded storefront embellishment, not for first-party order policy.

That is the main architectural lesson:

- linked Rust is the primary path for customer-owned commerce behavior
- WASM is the constrained path for runtime-installed behavior

## What The Package Does Not Do

The Shoppr WASM package does not own:

- checkout policy
- payment reconciliation
- account lifecycle
- operator workflows

Those stay in official modules and linked customer Rust.

That boundary is the point of the example.

## Adapt This For Your App

Use a Shoppr-style WASM package when you need:

- a bounded runtime-installed feature
- an explicit handler target
- a smaller trust surface than linked Rust

Do not use it just because “plugin” sounds attractive. If the logic is first-party and core to the
product, linked Rust is usually the better fit.

## Read Next

- [Customer Rust Vs Third-Party WASM](../../reference/customer-vs-wasm.md)
- [Linked Rust Backend](./linked-rust-backend.md)
