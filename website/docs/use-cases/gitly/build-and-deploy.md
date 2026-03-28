---
title: Gitly Build And Deploy
---

This page is about the customer-owned Coil lifecycle for a non-commerce app, with Gitly as the
supporting example.

Use it when you want to answer:

- what the customer binary should own
- what a good container bootstrap does
- how a third-party developer should run the app locally

## The Core Pattern

A customer app should own these lifecycle verbs itself:

- describe
- validate
- migrate
- publish assets
- serve
- up

That keeps the app teachable and runnable without forcing every developer to start from the root
platform CLI.

## Canonical Customer Binary Shape

Gitly’s binary in `apps/gitly/crates/gitly-bin/src/main.rs` provides a clear example:

```rust
enum Command {
    Describe,
    Validate,
    Assets { command: AssetsCommand },
    Migrate { command: MigrateCommand },
    Serve { bind: Option<String> },
    Up { bind: Option<String> },
    ExtensionChecksums,
    LinkedBackend { command: LinkedBackendCommand },
}
```

This is the shape to copy:

- keep app lifecycle explicit
- keep app-specific helper commands nearby
- avoid making third-party developers start with monorepo-only operator flows

## Canonical Container Bootstrap Pattern

A good app container should call the customer binary, not reimplement lifecycle logic in shell.

Gitly’s `apps/gitly/docker/entrypoint.sh` is the concrete example:

```sh
gitly --config "$CONFIG_PATH" migrate apply --yes
gitly --config "$CONFIG_PATH" assets publish
exec gitly --config "$CONFIG_PATH" up
```

This snippet is the main operational lesson on the page:

- migrations stay explicit
- asset publication stays explicit
- the final long-running process is still the customer binary

## Canonical Local Stack Pattern

The Docker Compose file should expose dependencies plainly.

Gitly’s `apps/gitly/docker-compose.yml` wires:

- `postgres`
- `redis`
- `minio`
- `minio-init`
- `app`

and passes the app the same env-backed config values it expects from `platform.dev.toml`, such as:

- `DATABASE_URL`
- `REDIS_URL`
- `OBJECT_STORE_URL`
- `COIL_COOKIE_SECRET`
- `COIL_CSRF_SECRET`

This is the pattern to copy for a one-command local stack.

## Gitly As The Supporting Example

### Customer lifecycle code

Read:

- `apps/gitly/crates/gitly-bin/src/main.rs`
- `apps/gitly/crates/gitly-app/src/lib.rs`

These files show:

- CLI surface
- runtime-plan composition
- app validation and migration ownership

### Container bootstrap

Read:

- `apps/gitly/docker/entrypoint.sh`
- `apps/gitly/docker-compose.yml`

These show:

- dependency waits
- migration and asset publication sequence
- app start handoff

### Local developer contract

Read:

- `apps/gitly/.env.example`
- `apps/gitly/README.md`

These show:

- local port overrides
- the self-contained `gitly.localhost` host contract for local development
- first-run local walkthrough
- non-Docker development path

## Practical Rules To Copy

- let the customer binary own the public lifecycle verbs
- let the container bootstrap call the customer binary
- publish theme assets explicitly before serving
- keep dependency wiring visible in Compose and `.env.example`

## Full Implementation Pointers

- `apps/gitly/crates/gitly-bin/src/main.rs`
- `apps/gitly/crates/gitly-app/src/lib.rs`
- `apps/gitly/docker/entrypoint.sh`
- `apps/gitly/docker-compose.yml`
- `apps/gitly/.env.example`
- `apps/gitly/README.md`

## Read Next

- [Extensions And Host APIs](./extensions-and-host-apis.md)
- [CLI Commands](../../reference/cli-commands.md)
- [Environment Variables](../../reference/environment-variables.md)
