---
title: Customer Project Layout
---

Coil’s preferred shape is a customer-owned Rust workspace that depends on Coil as upstream crates.

This page is for the moment when you want the exact project shape, not another abstract reminder
that “the customer workspace matters.”

## The Layout To Copy

Use a workspace that looks like this:

```text
tutorial-app/
  Cargo.toml
  app.toml
  platform.dev.toml
  docker-compose.yml
  crates/
    tutorial-app-app/
      Cargo.toml
      src/lib.rs
    tutorial-app-backend/
      Cargo.toml
      src/lib.rs
    tutorial-app-bin/
      Cargo.toml
      src/main.rs
  templates/
    layouts/
    pages/
    components/
  theme/
    assets/
  auth/
    tutorial-auth/
```

## The Root Workspace File

`Cargo.toml` should look like a normal Rust workspace, not a framework-owned monorepo stub:

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

[workspace.dependencies]
coil-all = "0.1"
coil-customer-sdk = "0.1"
serde = { version = "1", features = ["derive"] }
```

That one file already answers a key question: the customer project is the composition root, and
Coil is an upstream dependency.

## The Three Crates

### `crates/tutorial-app-bin/Cargo.toml`

```toml
[package]
name = "tutorial-app-bin"
version.workspace = true
edition.workspace = true

[dependencies]
tutorial-app-app = { path = "../tutorial-app-app" }
```

### `crates/tutorial-app-app/Cargo.toml`

```toml
[package]
name = "tutorial-app-app"
version.workspace = true
edition.workspace = true

[dependencies]
coil-all.workspace = true
coil-config = "0.1"
tutorial-app-backend = { path = "../tutorial-app-backend" }
```

### `crates/tutorial-app-backend/Cargo.toml`

```toml
[package]
name = "tutorial-app-backend"
version.workspace = true
edition.workspace = true

[dependencies]
coil-customer-sdk.workspace = true
```

## The App-Root Files

### `app.toml`

This is the app contract:

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

### `platform.dev.toml`

This is environment/runtime configuration:

```toml
[app]
name = "tutorial-app"
environment = "development"

[server]
bind = "127.0.0.1:8080"

[database]
mode = "postgres"
url = "postgres://postgres:postgres@127.0.0.1:5432/tutorial_app"

[cache]
mode = "redis"
url = "redis://127.0.0.1:6379"

[storage]
mode = "local"
local_root = ".coil/state"
```

### `docker-compose.yml`

This is the local dependency surface:

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

## The Full App Crate File

The app crate is where the workspace and the app root meet:

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

If your runtime composition is hard to find, the project shape is drifting.

## The Full Binary File

The binary should stay thin:

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

The binary owns lifecycle entry. The app crate owns composition.

## The Full Backend File

The backend crate should be present even before it contains much logic:

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

## Checkpoint

At this point a reviewer should be able to copy these files and produce the same workspace shape:

```text
Cargo.toml
app.toml
platform.dev.toml
docker-compose.yml
crates/tutorial-app-bin/Cargo.toml
crates/tutorial-app-bin/src/main.rs
crates/tutorial-app-app/Cargo.toml
crates/tutorial-app-app/src/lib.rs
crates/tutorial-app-backend/Cargo.toml
crates/tutorial-app-backend/src/lib.rs
```

They should also be able to run the workspace through the customer binary:

```bash
docker compose up -d
cargo run -p tutorial-app-bin -- validate
cargo run -p tutorial-app-bin -- serve
```

## What To Read Next

- [Linked Rust backends](linked-rust-backends.md)
- [Customer-root workspace](../core-concepts/customer-root-workspace.md)
- [Runtime and module composition](../core-concepts/runtime-and-module-composition.md)
- [Build and deploy](../operations/build-and-deploy.md)
