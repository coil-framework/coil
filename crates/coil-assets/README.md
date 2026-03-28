# coil-assets

`coil-assets` provides asset publication and delivery primitives for Coil.

It is used to publish theme assets, track release artefacts, and coordinate asset delivery with the configured storage layer.

## Install

```toml
[dependencies]
coil-assets = "0.1.0"
```

## When to use this crate directly

- You are composing Coil manually and need asset publication behaviour.
- You are building tooling around release artefacts or storage-backed theme assets.
- You are contributing to Coil’s asset pipeline.

## Related crates

- `coil-storage`: object storage integration used to persist published assets.
- `coil-runtime`: serves and references published assets at runtime.
- `coil-app`: coordinates customer themes and asset planning.

## Learn more

- Docs: https://coil.rs/docs
- Architecture: https://coil.rs/architecture
