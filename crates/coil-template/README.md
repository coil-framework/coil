# coil-template

`coil-template` is Coil’s server-side template parser and renderer.

It powers the HTML-first rendering model used across Coil applications and is responsible for parsing templates, expressions, and related rendering structures.

## Install

```toml
[dependencies]
coil-template = "0.1.0"
```

## When to use this crate directly

- You are composing Coil manually and want direct access to the template engine.
- You are building or extending template parsing and rendering behaviour.
- You are contributing to Coil’s HTML-first application model.

## Related crates

- `coil-runtime`: uses the template engine during live request handling.
- `coil-app`: coordinates customer application templates and theme structure.
- `coil-seo`: SEO output that is injected into rendered pages.

## Learn more

- Docs: https://coil.rs/docs/reference/template-language
- Architecture: https://coil.rs/architecture
