# coil-customer-sdk

`coil-customer-sdk` defines the stable extension interfaces for customer-owned Rust code in Coil.

If you are writing linked backend logic for a customer application, this is the public surface you should target rather than reaching into internal runtime crates.

## Install

```toml
[dependencies]
coil-customer-sdk = "0.1.0"
```

## When to use this crate

- You are building customer-owned Rust plugins or hook implementations.
- You want stable public traits instead of depending on internal runtime details.
- You are wiring custom business rules into a Coil application.

## Related crates

- `coil-rs`: batteries-included framework entrypoint for most applications.
- `coil-app`: customer application composition.
- `coil-runtime`: the runtime that executes customer-owned hooks.

## Learn more

- Docs: https://coil.rs/docs/getting-started/linked-rust-backends
- Architecture: https://coil.rs/architecture
