---
title: Customer Workspace Binaries
---

Coil customer apps ship their own CLI binary on purpose.

That is not duplication. It is the public lifecycle boundary for the customer app.

## Why A Customer Binary Exists

The platform CLI gives you generic operator workflows. The customer binary gives you the lifecycle
for one app.

That split matters because a third-party developer building a store should be able to run:

```bash
cd apps/shoppr
cargo run -p shoppr -- validate
```

without learning the whole monorepo first.

## One Binary Or Two?

There are two binaries:

- the platform CLI binary, built from the `coil-cli` crate, exposed here as `coil`
- the customer workspace binary, such as `shoppr` or `gitly`

They are related, but they are not the same thing.

The customer binary does not shell out to the platform CLI. It builds the customer workspace
bootstrap directly in Rust.

## What The Customer Binary Owns

Shoppr’s CLI entrypoint lives in:

- `apps/shoppr/crates/shoppr-bin/src/main.rs`

Gitly’s lives in:

- `apps/gitly/crates/gitly-bin/src/main.rs`

Those binaries own app-shaped commands such as:

- `describe`
- `validate`
- `assets publish`
- `migrate apply`
- `serve`
- `up`
- linked-backend or extension diagnostics that only make sense for that app

## Real `shoppr` Example

Actual help output:

```text
Usage: shoppr [OPTIONS] <COMMAND>

Commands:
  describe
  validate
  assets
  migrate
  serve
  up
  linked-backend
```

That is the public app lifecycle:

- `describe` explains the app shape
- `validate` checks composition, routes, jobs, and migration contracts
- `serve` runs the app
- `up` performs app-shaped bootstrap before serving

## Real `gitly` Example

Gitly adds an app-specific command:

- `extension-checksums`

That is exactly why customer binaries exist. Gitly has runtime-installed demo extensions, so the
customer CLI exposes a command that makes sense for Gitly and would be noise in the root platform
CLI.

## When To Use Which Binary

Use the customer binary when you want:

- the app’s own developer story
- app-local validation
- app-local serve and up flows
- app-specific diagnostics

Use the platform CLI when you want:

- generic auth diagnostics
- generic module operations
- generic import, TLS, cache, jobs, or release workflows

## Read Next

- [CLI Commands](./cli-commands/)
- [CLI Auth And Module Commands](./cli-auth-and-modules/)
- [CLI Migrations, Release, And Import](./cli-migrations-release-and-import/)
