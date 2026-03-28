# coil-cache

`coil-cache` provides caching primitives and cache backend integrations for Coil.

It is used by the runtime and platform layers to model cache behaviour, serialisation, and backend coordination.

## Install

```toml
[dependencies]
coil-cache = "0.1.0"
```

## When to use this crate directly

- You are composing Coil from lower-level crates and want explicit cache control.
- You are working on caching behaviour or backend integrations.
- You are building framework-level tooling rather than depending only on `coil-rs`.

## Related crates

- `coil-config`: selects and configures cache backends.
- `coil-runtime`: consumes cache services during live request handling.
- `coil-core`: aggregates cache behaviour with other shared runtime contracts.

## Learn more

- Docs: https://coil.rs/docs
- Architecture: https://coil.rs/architecture
