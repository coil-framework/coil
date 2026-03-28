# coil-runtime

`coil-runtime` is the lower-level HTTP runtime for Coil.

It serves requests, resolves sites and locales, executes templates, coordinates official modules, and hosts customer-owned Rust plugins and WASM extensions.

## Install

```toml
[dependencies]
coil-runtime = "0.1.0"
```

## When to use this crate directly

- You are composing Coil manually and need direct control over the runtime.
- You are contributing to request handling, rendering, or runtime integrations.
- You are building advanced framework-level integrations rather than starting from the top-level crate.

Most applications should begin with `coil = { package = "coil-rs", ... }`.

## Related crates

- `coil-core`: shared runtime contracts.
- `coil-app`: customer application composition and bootstrap planning.
- `coil-customer-sdk`: stable customer hook interfaces exposed into the runtime.

## Learn more

- Docs: https://coil.rs/docs/core-concepts/request-and-render-lifecycle
- Architecture: https://coil.rs/architecture
