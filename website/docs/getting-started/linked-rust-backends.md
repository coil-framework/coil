---
title: Linked Rust Backends
---

Davenda’s preferred customization model is linked customer Rust, not “go build a separate API and hope it lines up.”

## The Model

- Davenda core owns the runtime and official modules.
- The customer app owns a linked Rust backend crate.
- That backend crate plugs into Davenda through stable public APIs.
- Third-party integrations that should not participate in the build use WASM instead.

## Why It Matters

This gives customer teams:

- real Rust ergonomics
- compile-time safety
- first-party access through supported APIs
- fewer deployment boundaries

It also gives the platform a clearer separation between:

- customer-owned code
- official modules
- third-party runtime extensions

See:

- Shoppr for the ecommerce-oriented example
- chapter 96 in the architecture section for the deeper design record
