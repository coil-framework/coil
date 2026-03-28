# coil-tls

`coil-tls` provides TLS management primitives for Coil.

It is used for certificate material handling, renewal flows, validation support, and other framework-level TLS concerns.

## Install

```toml
[dependencies]
coil-tls = "0.1.0"
```

## When to use this crate directly

- You are composing Coil manually and need TLS support.
- You are building operational tooling around certificate lifecycle management.
- You are contributing to Coil’s TLS and certificate handling.

## Related crates

- `coil-config`: TLS configuration models.
- `coil-data`: persistence used for TLS state and related records.
- `coil-cli`: operational commands that surface TLS workflows.

## Learn more

- Docs: https://coil.rs/docs/operations/cache-tls-cutover-and-rollback
- Architecture: https://coil.rs/architecture
