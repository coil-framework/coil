# coil-a11y

`coil-a11y` contains accessibility primitives used by Coil’s rendering and content systems.

This crate is part of Coil’s platform contract around accessible product surfaces. Most applications will consume it indirectly through higher-level crates such as `coil-core`, `coil-runtime`, or the top-level `coil` crate.

## Install

```toml
[dependencies]
coil-a11y = "0.1.0"
```

## When to use this crate directly

- You are composing Coil manually from lower-level crates.
- You are working on rendering, content, or policy code that needs explicit accessibility metadata.
- You are extending internal framework behaviour rather than starting from the batteries-included entrypoint.

## Related crates

- `coil-i18n`: localisation primitives often used alongside accessibility-aware content.
- `coil-core`: aggregates accessibility, auth, caching, SEO, and template concerns into shared runtime contracts.

## Learn more

- Docs: https://coil.rs/docs
- Architecture: https://coil.rs/architecture
