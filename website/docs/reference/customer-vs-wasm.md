---
title: Customer Rust Vs Third-Party WASM
---

Davenda draws a hard line between customer-owned code and third-party extension code.

## Customer Rust

- compile-time linked
- first-party from the customer’s point of view
- accesses Davenda through stable public Rust APIs

## Third-Party WASM

- runtime-installed
- bounded by host APIs and capability grants
- better for marketplace or external plugin scenarios

This distinction is deliberate. It keeps the powerful customization path available without making the runtime extension boundary unsafe or incoherent.
