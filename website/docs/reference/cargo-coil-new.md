---
title: cargo coil new
---

`cargo coil new` creates a new customer-root workspace in a new directory.

This is the normal starting point for a new store or product.

## Default Behaviour

Interactive mode is the default:

```bash
cargo coil new my-store
```

That starts a prompt flow which asks for:

- project name
- display name
- default locale
- additional locales
- official modules
- optional extra sites

At the end, Coil writes `.coil/project.toml` and renders the managed workspace files into the new
directory.

## Non-Interactive Use

Use `--no-input` when you want a deterministic scaffold from automation or scripts:

```bash
cargo coil new my-store \
  --no-input \
  --default-locale en-GB \
  --locale fr-FR \
  --module cms \
  --module media \
  --module commerce \
  --module admin \
  --module ops
```

## Dependency Source

You can control how the generated workspace depends on Coil.

### Use the local checkout

```bash
cargo coil new my-store --source path --coil-path /path/to/coil
```

### Use crates.io

```bash
cargo coil new my-store --source crates-io
```

If neither flag is passed, `cargo coil` detects whether it is running from inside a Coil checkout
and prefers `path` dependencies when it can.

## Example End-To-End Flow

From a Coil checkout:

```bash
cargo run -p cargo-coil -- new my-store
cd my-store
docker compose up -d
export DATABASE_URL=postgres://coil:coil@127.0.0.1:5432/my-store
export REDIS_URL=redis://127.0.0.1:6379/0
export COIL_COOKIE_SECRET=replace-me-with-a-long-random-secret
export COIL_CSRF_SECRET=replace-me-with-a-long-random-secret
cargo run -p my-store -- validate
cargo run -p my-store -- serve
```

If `cargo-coil` is installed on your `PATH`, the same command becomes:

```bash
cargo coil new my-store
```

## Read Next

- [cargo coil init](./cargo-coil-init.md)
- [cargo coil apply](./cargo-coil-apply.md)
