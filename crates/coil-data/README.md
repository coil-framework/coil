# coil-data

`coil-data` provides data access and persistence primitives for Coil.

It owns shared database-facing abstractions used by higher-level modules and operational crates.

## Install

```toml
[dependencies]
coil-data = "0.1.0"
```

## When to use this crate directly

- You are composing Coil manually and need the shared data layer.
- You are building framework code that needs direct persistence primitives.
- You are contributing to the way Coil integrates with PostgreSQL and related storage concerns.

## Related crates

- `coil-config`: database and platform configuration models.
- `coil-runtime`: consumes the data layer during live request handling.
- `coil-jobs`: uses the data layer for queue state and coordination.

## Learn more

- Docs: https://coil.rs/docs/operations/database-migrations
- Architecture: https://coil.rs/architecture
