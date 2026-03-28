# coil-storage

`coil-storage` provides object-storage primitives for Coil.

It handles the storage-facing side of published assets, managed media, and other object-backed artefacts used by the platform.

## Install

```toml
[dependencies]
coil-storage = "0.1.0"
```

## When to use this crate directly

- You are composing Coil manually and need direct object storage integration.
- You are building tooling around asset or media storage.
- You are contributing to Coil’s storage backends or storage policy model.

## Related crates

- `coil-assets`: asset publication built on top of object storage.
- `coil-media`: richer media workflows that depend on object storage.
- `coil-config`: storage configuration models.

## Learn more

- Docs: https://coil.rs/docs/operations/asset-publication-and-cdn-delivery
- Architecture: https://coil.rs/architecture
