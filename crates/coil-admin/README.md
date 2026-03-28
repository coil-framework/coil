# coil-admin

`coil-admin` provides admin and operator-facing surfaces for Coil applications.

It is used to model and assemble the back-office parts of a Coil product: admin routes, privileged workflows, supporting auth requirements, and integrations with jobs and extension points.

## Install

```toml
[dependencies]
coil-admin = "0.1.0"
```

## When to use this crate directly

- You are composing Coil manually and want admin capabilities without the full top-level crate.
- You are extending or contributing to Coil’s admin surface area.
- You are building framework-level tooling around privileged workflows.

Most customer applications should start with `coil = { package = "coil-rs", ... }` instead.

## Related crates

- `coil-auth`: authorisation and policy modelling used by admin capabilities.
- `coil-jobs`: background work used by administrative workflows.
- `coil-wasm`: extension boundary used by admin-side integrations.

## Learn more

- Docs: https://coil.rs/docs
- Architecture: https://coil.rs/architecture
