# coil-jobs

`coil-jobs` provides background job and scheduler primitives for Coil.

It models queued work, retryable execution, scheduling, and the persistence needed to coordinate job state.

## Install

```toml
[dependencies]
coil-jobs = "0.1.0"
```

## When to use this crate directly

- You are composing Coil manually and want access to the jobs subsystem.
- You are building framework-level background work integration.
- You are contributing to schedulers, queue handling, or retry behaviour.

## Related crates

- `coil-config`: queue and scheduler configuration models.
- `coil-runtime`: live execution paths that enqueue or consume jobs.
- `coil-ops`: operational tooling built around jobs and release workflows.

## Learn more

- Docs: https://coil.rs/docs/operations/jobs-and-schedulers
- Architecture: https://coil.rs/architecture
