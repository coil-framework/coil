# coil-media

`coil-media` provides media-management capabilities for Coil applications.

It sits on top of assets, storage, auth, and data services to model richer media workflows inside a Coil product.

## Install

```toml
[dependencies]
coil-media = "0.1.0"
```

## When to use this crate directly

- You are composing Coil manually and want media support.
- You are extending or contributing to Coil’s media module.

Most applications that need media support should start with `coil-rs`.

## Related crates

- `coil-assets`: theme and published asset support.
- `coil-storage`: object storage integration.
- `coil-auth`: policy and capability modelling for media workflows.

## Learn more

- Docs: https://coil.rs/docs/reference/modules/media
- Architecture: https://coil.rs/architecture
