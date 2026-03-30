---
title: Understand the Runtime Shape
---

This chapter explains which generated files own which concerns before you start changing the UI.

The easiest way to understand Coil is to read the concrete files together, not memorize abstract
roles.

## Start With The Actual Files

For a generated `tutorial-app`, the important early files look like this.

### `app.toml`

This file declares what the app is:

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

What it owns:

- app identity
- domains and locales
- enabled official modules
- theme roots
- auth package selection

### `platform.dev.toml`

This file declares how the app runs in development:

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

What it owns:

- HTTP bind
- backend connections
- storage mode
- local-runtime behaviour

### `crates/tutorial-app-app/src/lib.rs`

This file composes the app:

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

What it owns:

- loading config
- choosing official modules
- registering customer plugins
- building and running the runtime

### `crates/tutorial-app-bin/src/main.rs`

This file is the operational entrypoint:

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

What it owns:

- process entry
- stdout/stderr and exit code
- invoking the app crate’s command surface

### `crates/tutorial-app-backend/src/lib.rs`

This is where customer-specific Rust starts:

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

What it owns:

- linked customer hooks
- product-specific backend rules
- request-time shaping later in the tutorial

### `templates/pages/home.html`

Even before deeper UI work, the generated app should already have a real page file:

```html
<!doctype html>
<html xmlns:coil="https://coil.rs" coil:replace="~{layouts/base}">
  <body>
    <section>
      <h1>Tutorial App</h1>
      <p>Welcome to the generated customer app.</p>
    </section>
  </body>
</html>
```

### `theme/assets/site.css`

The generated theme layer should also already exist as a concrete asset:

```css
body {
  margin: 0;
  font: 16px/1.5 sans-serif;
}
```

## The Ownership Table

Keep this mapping in your head:

| Concern | File or layer |
| --- | --- |
| app identity, sites, modules | `app.toml` |
| local runtime behaviour | `platform.dev.toml` |
| runtime composition | `crates/tutorial-app-app/src/lib.rs` |
| CLI lifecycle entry | `crates/tutorial-app-bin/src/main.rs` |
| customer backend rules | `crates/tutorial-app-backend/src/lib.rs` |
| HTML shell | `templates/` |
| CSS, images, assets | `theme/` |

## Checkpoint

You are ready to continue when you can point at these concrete files and explain why each exists:

```text
app.toml
platform.dev.toml
crates/tutorial-app-app/src/lib.rs
crates/tutorial-app-bin/src/main.rs
crates/tutorial-app-backend/src/lib.rs
templates/pages/home.html
theme/assets/site.css
```

## Reference Companion

- [App TOML](../reference/app-toml.md)
- [Platform Config](../reference/platform-config.md)
- [Composition](../reference/composition.md)
- [Customer project layout](customer-project-layout.md)
- [Linked Rust backends](linked-rust-backends.md)

## What To Read Next

- [Build the Base Theme](build-the-base-theme.md)
