# coil-cli

`coil-cli` is the command-line tooling crate for Coil.

It powers the `coil` command used for validation, local development, imports, migrations, asset publication, jobs, storage checks, and release operations.

## Install the CLI

```bash
cargo install coil-cli --bin coil
```

## Use as a library

```toml
[dependencies]
coil-cli = "0.1.0"
```

## When to use this crate directly

- You want the `coil` command on your machine or CI runner.
- You are building a customer binary that reuses Coil’s command surface.
- You are extending or contributing to Coil’s operational tooling.

## Related crates

- `coil-app`: customer application composition and planning.
- `coil-runtime`: runtime services and live execution.
- `coil-ops`: operational and release-management primitives.

## Learn more

- Operations docs: https://coil.rs/docs/operations/build-and-deploy
- CLI reference: https://coil.rs/docs/reference/cli-commands
