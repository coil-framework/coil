---
title: Getting Started Tutorial
---

This tutorial builds one customer-owned Coil application in stages.

The rule for this section is simple: when a chapter introduces a file, the chapter should show you
the file.

## The Tutorial App

The examples use a generic workspace named `tutorial-app`.

The first working checkpoint looks like this:

```text
tutorial-app/
  Cargo.toml
  app.toml
  platform.dev.toml
  docker-compose.yml
  crates/
    tutorial-app-app/
    tutorial-app-backend/
    tutorial-app-bin/
  templates/
  theme/
  auth/
```

The generated root `Cargo.toml` should already look like a real Rust workspace:

```toml
[workspace]
members = [
  "crates/tutorial-app-app",
  "crates/tutorial-app-backend",
  "crates/tutorial-app-bin",
]
resolver = "2"

[workspace.package]
edition = "2021"
version = "0.1.0"
license = "MIT OR Apache-2.0"

[workspace.dependencies]
coil-all = "0.1"
coil-customer-sdk = "0.1"
serde = { version = "1", features = ["derive"] }
toml = "0.8"
```

The generated `app.toml` should already look like an app contract, not a placeholder:

```toml
name = "tutorial-app"
display_name = "Tutorial App"

[domains]
canonical = "www.127.0.0.1.nip.io"
additional = []

[i18n]
default_locale = "en-GB"
supported_locales = ["en-GB"]

[theme]
asset_roots = ["theme/assets"]

[auth]
package = "tutorial-auth"

[[modules]]
name = "admin"

[[modules]]
name = "cms"

[[modules]]
name = "commerce"
```

The matching `platform.dev.toml` should already be runnable:

```toml
[app]
name = "tutorial-app"
environment = "development"

[server]
bind = "127.0.0.1:8080"

[i18n]
default_locale = "en-GB"
supported_locales = ["en-GB"]
localized_routes = true

[seo]
canonical_host = "www.127.0.0.1.nip.io:8080"

[database]
mode = "postgres"
url = "postgres://postgres:postgres@127.0.0.1:5432/tutorial_app"

[cache]
mode = "redis"
url = "redis://127.0.0.1:6379"

[jobs]
mode = "postgres"

[storage]
mode = "local"
local_root = ".coil/state"
```

## The Working Loop

Throughout the tutorial you will repeat this loop:

```bash
docker compose up -d
cargo run -p tutorial-app-bin -- validate
cargo run -p tutorial-app-bin -- serve
```

When a chapter adds or changes a file, rerun that loop before moving on.

## Where To Begin

Read these chapters in order:

- [What You Are Building](what-you-are-building.md)
- [Create the Project](create-the-project.md)
- [Understand the Runtime Shape](understand-the-runtime-shape.md)
- [Build the Base Theme](build-the-base-theme.md)
- [Add Sites, Markets, and Locales](add-sites-markets-and-locales.md)
- [Add a Real Content Model](add-a-real-content-model.md)
- [Build Reusable Blocks](build-reusable-blocks.md)
- [Add Dynamic Blocks](add-dynamic-blocks.md)

Supporting pages:

- [Customer Project Layout](customer-project-layout.md)
- [Linked Rust Backends](linked-rust-backends.md)

## Fast Bootstrap

If you only want the shortest smoke test:

```bash
cargo install cargo-coil --locked
cargo coil new tutorial-app
cd tutorial-app
docker compose up -d
cargo run -p tutorial-app-bin -- validate
cargo run -p tutorial-app-bin -- serve
```

That gets you a running app. The rest of this section is where the file ownership and runtime
shape become understandable.

## Checkpoint

From a fresh generated workspace, these commands should all succeed:

```bash
docker compose up -d
cargo run -p tutorial-app-bin -- validate
cargo run -p tutorial-app-bin -- serve
```

At this point you should have a running app and these concrete files on disk:

```text
Cargo.toml
app.toml
platform.dev.toml
docker-compose.yml
crates/tutorial-app-app/src/lib.rs
crates/tutorial-app-backend/src/lib.rs
crates/tutorial-app-bin/src/main.rs
```
