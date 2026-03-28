# coil-config

`coil-config` contains the configuration models and loaders used across Coil.

If you need to parse, validate, or reason about `platform.toml`, `platform.dev.toml`, or related platform-level configuration, this is the crate that owns those shapes.

## Install

```toml
[dependencies]
coil-config = "0.1.0"
```

## When to use this crate directly

- You are building tooling around Coil configuration files.
- You are composing Coil manually and want direct access to configuration models.
- You are contributing to the platform configuration layer.

## Related crates

- `coil-app`: customer application composition built on top of configuration models.
- `coil-runtime`: consumes resolved configuration at runtime.
- `coil-storage`, `coil-jobs`, and `coil-tls`: backend-specific crates that depend on shared config models.

## Learn more

- Config docs: https://coil.rs/docs/reference/platform-config
- Architecture: https://coil.rs/architecture
