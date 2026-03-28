# coil-ops

`coil-ops` provides operational and release-management capabilities for Coil.

It contains the primitives used for deployment-time checks, operational state, and release-oriented workflows such as cutover support.

## Install

```toml
[dependencies]
coil-ops = "0.1.0"
```

## When to use this crate directly

- You are composing Coil manually and need operational primitives.
- You are building release-management or operator tooling.
- You are contributing to Coil’s deployment and cutover model.

## Related crates

- `coil-jobs`: queue-aware operational workflows.
- `coil-auth`: privileged access control for operator actions.
- `coil-cli`: command surface that exposes many ops workflows to users.

## Learn more

- Docs: https://coil.rs/docs/operations/cache-tls-cutover-and-rollback
- Architecture: https://coil.rs/architecture
