# coil-memberships

`coil-memberships` provides membership and subscription capabilities for Coil applications.

It coordinates auth, commerce, shared core contracts, and background work for member-aware product behaviour.

## Install

```toml
[dependencies]
coil-memberships = "0.1.0"
```

## When to use this crate directly

- You are composing Coil manually and want membership support.
- You are extending or contributing to Coil’s memberships module.

## Related crates

- `coil-commerce`: billing and transactional behaviour used by memberships.
- `coil-auth`: member-aware access control.
- `coil-jobs`: background work for subscription and membership flows.

## Learn more

- Docs: https://coil.rs/docs/reference/modules/memberships
- Architecture: https://coil.rs/architecture
