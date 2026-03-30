---
title: Customer Project Layout
---

This page shows the exact workspace shape the tutorial is using and what each file contributes.

## The Workspace Shape

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

This split matters because the tutorial is teaching a customer-owned app, not a monolithic starter
crate.

## Root `Cargo.toml`

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

This file does two things:

- it makes the three customer crates build together
- it centralizes shared dependency versions

## `crates/tutorial-app-bin/Cargo.toml`

```toml
[package]
name = "tutorial-app-bin"
version.workspace = true
edition.workspace = true

[dependencies]
tutorial-app-app = { path = "../tutorial-app-app" }
```

This crate depends only on the app crate because it is just the process entrypoint.

## `crates/tutorial-app-app/Cargo.toml`

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

This crate depends on Coil plus the customer backend because it composes the runtime.

## `crates/tutorial-app-backend/Cargo.toml`

```toml
[package]
name = "tutorial-app-backend"
version.workspace = true
edition.workspace = true

[dependencies]
coil-customer-sdk.workspace = true
```

This crate depends only on the stable customer SDK because it should not need runtime internals.

## `app.toml`

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

This file defines product structure:

- product identity
- public domains
- locales
- enabled official modules
- theme and auth roots

## `platform.dev.toml`

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

This file defines how the app runs in development:

- which port to bind
- where database and cache backends live
- where local state is stored

## `docker-compose.yml`

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

This file starts the local services the platform config points at.

## `crates/tutorial-app-app/src/lib.rs`

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

This file enables the app’s runtime behavior:

- loads config during validation
- links the customer backend
- registers official modules
- serves through the app-owned composition root

## `crates/tutorial-app-bin/src/main.rs`

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

This file makes the app runnable as a normal binary.

## `crates/tutorial-app-backend/src/lib.rs`

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

This file is the starting point for customer-owned backend rules.

## Checkpoint

Run:

```bash
docker compose up -d
cargo run -p tutorial-app-bin -- validate
cargo run -p tutorial-app-bin -- serve
```

At this point you should be able to explain which file owns product structure, runtime config,
composition, process entry, and customer backend behavior.
