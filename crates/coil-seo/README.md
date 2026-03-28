# coil-seo

`coil-seo` provides SEO primitives for Coil applications.

It models the data used to build canonical URLs, alternate locales, discoverability metadata, and other SEO-facing output.

## Install

```toml
[dependencies]
coil-seo = "0.1.0"
```

## When to use this crate directly

- You are composing Coil manually and want direct access to SEO metadata structures.
- You are contributing to the platform’s SEO behaviour.
- You are building framework-level integrations around discoverability output.

## Related crates

- `coil-i18n`: locale-aware SEO support.
- `coil-runtime`: injects SEO output into live rendered responses.

## Learn more

- Docs: https://coil.rs/docs/reference/seo
- Architecture: https://coil.rs/architecture
