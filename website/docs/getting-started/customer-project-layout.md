---
title: Customer Project Layout
---

Davenda’s preferred shape is a customer-owned project that depends on Davenda as upstream crates.

That customer project owns:

- the binary
- the app manifest
- templates and theme assets
- auth mappings
- customer-specific Rust logic
- optional third-party WASM extensions

## Recommended Shape

```text
customer-store/
  Cargo.toml
  crates/
    my-store-app/
    my-store-backend/
    my-store-bin/
  app/
    app.toml
    platform.toml
    templates/
    theme/
    auth/
    extensions/
```

## Why This Shape

- it keeps Davenda upgradeable as a normal dependency
- it gives customer code first-party compile-time access through public SDKs
- it avoids treating customer business logic like an afterthought sidecar
- it keeps third-party extensions in a separate, bounded runtime model

Shoppr demonstrates this shape directly in `apps/shoppr/`.
