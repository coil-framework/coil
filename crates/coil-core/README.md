# coil-core

`coil-core` contains the shared runtime contracts and platform primitives at the heart of Coil.

It aggregates concerns such as auth, caching, localisation, observability, SEO, templating, TLS, and extension wiring into the core interfaces used by higher-level crates.

## Install

```toml
[dependencies]
coil-core = "0.1.0"
```

## When to use this crate directly

- You are composing Coil from lower-level crates.
- You are building framework-level features against the shared core interfaces.
- You are contributing to the platform’s central contracts.

Most applications should begin with `coil-rs` rather than depending on `coil-core` directly.

## Related crates

- `coil-runtime`: the live HTTP runtime built on top of these contracts.
- `coil-app`: customer application composition that feeds the runtime.
- `coil-customer-sdk`: stable public hooks for customer-owned code.

## Learn more

- Docs: https://coil.rs/docs/core-concepts/runtime-and-module-composition
- Architecture: https://coil.rs/architecture
