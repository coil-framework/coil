# coil-app

`coil-app` contains customer application composition helpers for Coil.

This crate sits between customer-owned application code and the lower-level runtime. It is responsible for shaping application manifests, build planning, template/runtime coordination, and customer-app bootstrap concerns.

## Install

```toml
[dependencies]
coil-app = "0.1.0"
```

## When to use this crate directly

- You are building a custom customer binary on top of Coil.
- You want manual control over application composition instead of relying only on the top-level `coil` crate.
- You are contributing to the customer application model itself.

For most applications, `coil-rs` is still the right first dependency.

## Related crates

- `coil-customer-sdk`: stable interfaces for customer-owned backend logic.
- `coil-runtime`: the HTTP runtime that serves the composed customer application.
- `coil-template`: the server-side template engine used by customer themes.

## Learn more

- Docs: https://coil.rs/docs
- Architecture: https://coil.rs/architecture
