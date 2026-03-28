# coil-import

`coil-import` provides import-manifest and reporting support for Coil.

It is used for structured import flows that bring content, catalogue, or other operational data into a Coil application in a predictable way.

## Install

```toml
[dependencies]
coil-import = "0.1.0"
```

## When to use this crate directly

- You are building import tooling around Coil.
- You need to plan or execute import flows outside the top-level CLI.
- You are contributing to the platform’s import subsystem.

## Related crates

- `coil-report`: shared report structures returned by import flows.
- `coil-cli`: operational command surface that exposes imports to users.

## Learn more

- Docs: https://coil.rs/docs/operations/build-and-deploy
- Architecture: https://coil.rs/architecture
