---
title: Cargo Coil Overview
---

`cargo coil` is the project lifecycle CLI for Coil customer applications.

Install it like any other Cargo subcommand:

```bash
cargo install cargo-coil --locked
```

Use it when you need to create or evolve a customer-root workspace:

- initialise a new store or product workspace
- persist the intended project shape in `.coil/project.toml`
- regenerate managed files safely
- add modules, sites, and locales without hand-editing every file

This is a different concern from the root `coil` binary and from a customer workspace binary such
as `shoppr`.

## The Three CLI Layers

Coil now has three distinct CLI surfaces.

### `cargo coil`

This is the project generator and editor.

Use it for:

- `new`
- `init`
- `apply`
- `doctor`
- `module add|remove`
- `site add`
- `locale add`

This CLI owns the customer workspace shape itself.

### `coil`

This is the platform operator CLI.

Use it for:

- auth inspection
- module inspection
- jobs
- cache
- storage
- TLS
- migration and release planning

This CLI owns platform-wide operational workflows.

### Customer binaries such as `shoppr` or `my-store`

These are app-shaped binaries produced by the customer workspace.

Use them for:

- `validate`
- `serve`
- `up`
- app-specific diagnostics

These binaries own the lifecycle of one actual application.

## How `cargo coil` Works

`cargo coil` is not a one-shot file dump. It works from a project descriptor stored at:

```text
.coil/project.toml
```

That file records the intended project shape:

- project name and display name
- enabled modules
- locales
- sites
- dependency source
- whether linked Rust and extension directories are enabled

The command flow is:

1. gather intent through interactive prompts or flags
2. write `.coil/project.toml`
3. render the managed workspace files from that descriptor
4. let future edits update the descriptor and re-apply

That is why `cargo coil apply` and `cargo coil doctor` exist. The descriptor is the durable source
of truth.

## Dependency Resolution

`cargo coil` supports two dependency sources:

- `crates-io`
- `path`

When you run `cargo coil` from inside a Coil checkout, it automatically prefers local `path`
dependencies so you can use the generator immediately from the repository.

When you run it outside the Coil checkout, it falls back to crates.io dependencies unless you
explicitly pass:

```bash
cargo coil new my-store --source path --coil-path /path/to/coil
```

For a fully registry-backed new project, the generated workspace also expects the public framework
crates such as `coil-rs` and `coil-customer-sdk` to be available on crates.io.

## Command Map

- [cargo coil new](./cargo-coil-new.md)
- [cargo coil init](./cargo-coil-init.md)
- [cargo coil apply](./cargo-coil-apply.md)
- [cargo coil doctor](./cargo-coil-doctor.md)
- [cargo coil module add and remove](./cargo-coil-module.md)
- [cargo coil site add](./cargo-coil-site.md)
- [cargo coil locale add](./cargo-coil-locale.md)
