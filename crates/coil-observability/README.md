# coil-observability

`coil-observability` provides observability primitives for Coil.

It contains shared structures and integration points used for health, readiness, metrics, tracing, and related platform diagnostics.

## Install

```toml
[dependencies]
coil-observability = "0.1.0"
```

## When to use this crate directly

- You are composing Coil manually and want direct access to observability primitives.
- You are contributing to the platform’s health, readiness, metrics, or tracing story.
- You are building framework-level tooling around operational visibility.

## Related crates

- `coil-config`: configuration for observability-related behaviour.
- `coil-runtime`: live request/runtime surfaces that emit operational state.
- `coil-ops`: operator-facing flows built around runtime evidence.

## Learn more

- Docs: https://coil.rs/docs/operations/observability
- Architecture: https://coil.rs/architecture
