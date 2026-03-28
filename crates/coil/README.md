# coil-rs

`coil-rs` is the published package for the main `coil` crate.

Start here if you want the batteries-included Coil experience: application bootstrap, official modules, customer-owned Rust hooks, and the default runtime story in one dependency.

## Install

```toml
[dependencies]
coil = { package = "coil-rs", version = "0.1.0" }
```

## Basic shape

```rust
fn main() -> Result<(), anyhow::Error> {
    coil::builder()
        .with_customer_plugin(my_store_backend::plugin())
        .run_from_env()
}
```

## When to use this crate

- You are building a new Coil application and want the default entrypoint.
- You want the official modules without manually composing lower-level crates.
- You want customer-owned Rust code linked directly into the application.

## Related crates

- `coil-runtime`: the lower-level HTTP runtime used underneath the top-level builder.
- `coil-customer-sdk`: stable traits and hook interfaces for customer-owned code.
- `coil-cli`: command-line tooling for validation, operations, release work, and local development.

## Learn more

- Docs: https://coil.rs/docs
- Architecture: https://coil.rs/architecture
- Repository: https://github.com/coil-framework/coil
