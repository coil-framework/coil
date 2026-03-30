---
title: Create the Project
---

This chapter gets a generated customer workspace onto disk and to the first runnable checkpoint.

The important thing to notice is not just that `cargo coil new` creates files. It creates the
customer-root structure that the rest of the tutorial keeps extending.

## What You Will End This Chapter With

- a generated Rust workspace
- a customer binary you can run directly
- checked-in app config and templates
- local infrastructure running through Docker Compose
- a working `validate` and `serve` loop

## Generate The Project

Start with the generator:

```bash
cargo install cargo-coil --locked
cargo coil new tutorial-app
cd tutorial-app
```

From this point on, the tutorial assumes the workspace is named `tutorial-app`.

## What The Generated Files Should Look Like

The exact output can evolve, but a serious generated project should already look like a real Rust
workspace, not a one-file demo.

### Root `Cargo.toml`

This file establishes the customer workspace and keeps the application crates together:

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

### Customer binary: `crates/tutorial-app-bin/src/main.rs`

This is the command surface you run in development and later in operations:

```rust
use std::process::ExitCode;

fn main() -> ExitCode {
    match tutorial_app_app::run_from_args(std::env::args()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
```

### Customer app/bootstrap crate: `crates/tutorial-app-app/src/lib.rs`

This is the composition root. It loads config, links modules, links your customer backend, and runs
the runtime:

```rust
use coil_all::modules;
use coil_config::PlatformConfig;

pub fn run_from_args(
    args: impl IntoIterator<Item = String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut args = args.into_iter();
    let _program = args.next();
    match args.next().as_deref() {
        Some("validate") => {
            let _config = PlatformConfig::from_file("platform.dev.toml")?;
            Ok(())
        }
        Some("serve") => {
            coil_all::builder()
                .with_customer_plugin(tutorial_app_backend::TutorialAppPlugin)
                .with_module(modules::admin())
                .with_module(modules::cms())
                .with_module(modules::commerce())
                .run_from_env()?;
            Ok(())
        }
        other => Err(format!("unknown command: {:?}", other).into()),
    }
}
```

### Linked customer backend: `crates/tutorial-app-backend/src/lib.rs`

This crate starts small. It exists from day one so customer-owned Rust has a first-class place to
live:

```rust
use coil_customer_sdk::{CustomerBackendPlugin, CustomerHookRegistry};

pub struct TutorialAppPlugin;

impl CustomerBackendPlugin for TutorialAppPlugin {
    fn register(
        &self,
        _registry: &mut dyn CustomerHookRegistry,
    ) -> Result<(), coil_customer_sdk::BackendError> {
        Ok(())
    }
}
```

### App manifest: `app.toml`

This file describes what the app is:

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

### Local runtime config: `platform.dev.toml`

This file describes how the app runs in development:

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

### Local dependencies: `docker-compose.yml`

The generated app should also give you local infrastructure:

```yaml
services:
  postgres:
    image: postgres:16
    environment:
      POSTGRES_PASSWORD: postgres
      POSTGRES_DB: tutorial_app
    ports:
      - "5432:5432"

  redis:
    image: redis:7
    ports:
      - "6379:6379"
```

## Start Local Infrastructure

Bring up the generated dependencies:

```bash
docker compose up -d
```

At this point you should have the support services the generated config expects, not just a Rust
workspace sitting on disk.

## Validate The Generated App

Run validation before serving:

```bash
cargo run -p tutorial-app-bin -- validate
```

That should prove the config and workspace are internally coherent before you boot the server.

## Start The App

Now run the app through its own binary:

```bash
cargo run -p tutorial-app-bin -- serve
```

Open the generated site in the browser. Do not start editing yet. First verify that the command you
just ran came from your customer binary, not from a hidden framework dev server.

## Checkpoint

At the end of this chapter, these files should exist and these commands should work:

```text
Cargo.toml
app.toml
platform.dev.toml
docker-compose.yml
crates/tutorial-app-app/src/lib.rs
crates/tutorial-app-backend/src/lib.rs
crates/tutorial-app-bin/src/main.rs
```

```bash
docker compose up -d
cargo run -p tutorial-app-bin -- validate
cargo run -p tutorial-app-bin -- serve
```

## What To Read Next

- [Understand the Runtime Shape](understand-the-runtime-shape.md)
