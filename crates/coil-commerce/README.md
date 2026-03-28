# coil-commerce

`coil-commerce` provides commerce capabilities for Coil applications.

It contains the platform-level primitives used for catalogue, pricing, cart, checkout, and related commerce flows.

## Install

```toml
[dependencies]
coil-commerce = "0.1.0"
```

## When to use this crate directly

- You are composing Coil manually for a commerce-focused application.
- You are extending or contributing to Coil’s commerce layer.
- You need the commerce module without the full top-level bundle.

## Related crates

- `coil-core`: shared runtime contracts used by commerce flows.
- `coil-data`: persistence and database integration.
- `coil-jobs`: background work used by commerce operations.

## Learn more

- Docs: https://coil.rs/docs/reference/modules/commerce
- Shoppr guide: https://coil.rs/docs/use-cases/shoppr/overview
