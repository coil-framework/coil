---
title: Shoppr Overview
---

Shoppr is the primary way to learn Davenda through ecommerce.

It shows:

- multi-market host resolution
- multi-locale routing
- catalog and merchandising flows
- account and membership flows
- checkout and payments
- linked customer Rust
- third-party WASM
- admin and operator surfaces

## Why Shoppr Comes First

Ecommerce forces the framework to prove itself against the hard problems:

- product and content changes at pace
- money movement
- customer identity
- fulfillment and inventory boundaries
- operations and observability

If a framework can survive that shape cleanly, it is usually strong enough for simpler products too.

## What To Open

- `/` for the flagship storefront shell
- `/shop` and `/shop/collections` for browse and merchandising
- `/shop/products/...` for the interactive product page pattern
- `/cart` and `/checkout` for public commerce flows
- `/account` for customer continuity
- `/admin` and `/__dev` for developer inspection
