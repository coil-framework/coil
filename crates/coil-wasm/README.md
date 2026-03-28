# coil-wasm

`coil-wasm` provides the WASM extension runtime and host APIs for Coil.

It is the boundary used for third-party runtime extensions: loading extension modules, exposing host services, and coordinating execution within Coil.

## Install

```toml
[dependencies]
coil-wasm = "0.1.0"
```

## When to use this crate directly

- You are building or hosting third-party WASM extensions for Coil.
- You are extending the runtime-side WASM host boundary.
- You are contributing to Coil’s extension model.

If you own the application itself, prefer `coil-customer-sdk` for linked Rust customisation before reaching for WASM.

## Related crates

- `coil-customer-sdk`: linked Rust extension path for customer-owned code.
- `coil-runtime`: hosts and executes WASM extensions at runtime.
- `coil-admin`: admin-side extension integration points.

## Learn more

- Docs: https://coil.rs/docs/reference/wasm-host-apis
- Architecture: https://coil.rs/architecture
