# coil-auth

`coil-auth` provides Coil’s authorisation models, auth package handling, and integration with relationship-based access control.

It is the crate to use when you need to work with Coil auth schemas, package loading, bindings, or custom policy composition.

## Install

```toml
[dependencies]
coil-auth = "0.1.0"
```

## When to use this crate directly

- You are defining or validating auth packages.
- You are building framework-level auth tooling or diagnostics.
- You are composing Coil manually and want direct access to the auth model layer.

## Related crates

- `zanzibar`: the underlying relationship-based access control engine used by Coil.
- `coil-config`: platform configuration consumed by auth package loading.
- `coil-data`: data-layer support used by live auth integration.

## Learn more

- Auth docs: https://coil.rs/docs/reference/auth-overview
- Architecture: https://coil.rs/architecture
