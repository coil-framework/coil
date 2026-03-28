# coil-report

`coil-report` provides report structures shared across Coil tooling.

It is used when commands or subsystems need to return structured, human-readable operational or validation output.

## Install

```toml
[dependencies]
coil-report = "0.1.0"
```

## When to use this crate directly

- You are building tooling around Coil’s reporting model.
- You are composing operational or validation flows outside the top-level CLI.
- You are contributing to shared report types used across the workspace.

## Related crates

- `coil-import`: import reporting.
- `coil-cli`: user-facing command surface that renders many reports.

## Learn more

- Docs: https://coil.rs/docs/reference/cli-commands
- Architecture: https://coil.rs/architecture
