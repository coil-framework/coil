---
title: WASM Extensions
---

Shoppr also demonstrates the runtime-installed WASM path so developers can see how bounded extension behavior differs from linked customer Rust.

## What Shoppr Uses WASM For

The checked-in example is a waitlist-oriented extension package under `extensions/shoppr-waitlist-tools`.

This is intentionally narrower than the linked backend path. The extension exists to show:

- package metadata
- runtime installation
- explicit handler targets
- a lower-trust boundary than customer-owned linked Rust

## Why Davenda Keeps This Separate

Shoppr demonstrates both extension models because they solve different problems:

- linked Rust is for customer-owned first-party logic
- WASM is for bounded runtime-installed behavior

If those two models are blurred together, the security and operability story becomes weak quickly.

## What To Read Next

- [Customer Rust Vs Third-Party WASM](../../reference/customer-vs-wasm.md)
- [Linked Rust Backend](./linked-rust-backend.md)
