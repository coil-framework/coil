---
title: Linked Rust Backend
---

Shoppr is the main example of customer-owned Rust business logic compiled directly into the application.

## The Two Layers In Shoppr

Shoppr separates its linked backend into:

- `crates/shoppr-backend`
- `backend/shoppr-loyalty-backend`

The first is the Davenda-facing linked plugin crate.

The second contains the customer-owned domain logic the plugin wraps.

That split is useful because it keeps the Davenda integration seam clear while still allowing a richer internal business-rules library.

## What The Linked Backend Handles

The Shoppr backend is used for first-party behavior such as:

- checkout review
- verified webhook decisions
- loyalty or CRM-style customer routing
- product-specific business rules that belong to the store, not to the framework

This is the primary customization path for customer-owned behavior.

## Why This Is Better Than A Sidecar By Default

For ordinary first-party store logic, a linked backend is better because:

- it compiles with the application
- it shares one deployment artifact
- it participates through supported typed boundaries
- it avoids inventing a separate API surface unnecessarily

Use a separate service only when the operational boundary is real.

## What To Read Next

- [Linked Rust Backends](../../getting-started/linked-rust-backends.md)
- [Customer Rust Vs Third-Party WASM](../../reference/customer-vs-wasm.md)
- [Shoppr WASM Extensions](./wasm-extensions.md)
