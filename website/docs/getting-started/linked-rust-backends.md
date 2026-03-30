---
title: Linked Rust Backends
---

Coil’s preferred customization model is linked customer Rust.

This page shows the actual files you should expect to write when the tutorial starts introducing
customer-owned backend rules.

## The Smallest Linked Backend

Start with a dedicated crate.

### `crates/tutorial-app-backend/Cargo.toml`

```toml
[package]
name = "tutorial-app-backend"
version.workspace = true
edition.workspace = true

[dependencies]
coil-customer-sdk.workspace = true
```

### `crates/tutorial-app-backend/src/lib.rs`

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

That crate exists before you need complicated logic. It gives the customer-owned backend lane a
real place in the project.

## The App Crate That Links It

The app crate wires it into runtime composition.

### `crates/tutorial-app-app/src/lib.rs`

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

The important point is visible in code: customer logic is linked and registered explicitly. It is
not a hidden plugin directory or an ambient runtime script.

## Replace The Backend File With A Real Hook

Once you need customer rules, the same crate grows into hook implementations.

### `crates/tutorial-app-backend/src/lib.rs`

```rust
use coil_customer_sdk::{
    CmsHooks, CmsPageDraft, CmsPublishDecision, CustomerBackendPlugin, CustomerHookRegistry,
};

#[derive(Default)]
struct TutorialCmsHooks;

impl CmsHooks for TutorialCmsHooks {
    fn validate_page_publish(
        &self,
        draft: &CmsPageDraft,
    ) -> Result<CmsPublishDecision, coil_customer_sdk::BackendError> {
        if draft.slug.starts_with("internal-") {
            return Ok(CmsPublishDecision::reject(
                "internal pages cannot be published to the public site",
            ));
        }
        Ok(CmsPublishDecision::allow())
    }
}

pub struct TutorialAppPlugin;

impl CustomerBackendPlugin for TutorialAppPlugin {
    fn register(
        &self,
        registry: &mut dyn CustomerHookRegistry,
    ) -> Result<(), coil_customer_sdk::BackendError> {
        registry.register_cms_hooks(Box::new(TutorialCmsHooks::default()));
        Ok(())
    }
}
```

This is the pattern the tutorial eventually builds toward:

- customer code lives in a normal crate
- the app crate registers it
- the runtime calls it through stable facades and hook traits

The same pattern is what later chapters use for dynamic blocks and CMS editorial rules. When the
CMS admin starts exposing page settings, ordered blocks, and reusable shared blocks, the linked
backend remains the place for product-specific publish validation and runtime shaping.

## Checkpoint

At this point the linked-backend story should be visible in these concrete files:

```text
crates/tutorial-app-backend/Cargo.toml
crates/tutorial-app-backend/src/lib.rs
crates/tutorial-app-app/src/lib.rs
```

A reviewer should be able to answer:

- where customer-owned Rust lives
- where it gets linked into the app
- which file owns product-specific publish rules

They should also be able to run the app with the linked backend in place:

```bash
docker compose up -d
cargo run -p tutorial-app-bin -- validate
cargo run -p tutorial-app-bin -- serve
```

## What To Read Next

- [Customer project layout](customer-project-layout.md)
- [Customer-root workspace](../core-concepts/customer-root-workspace.md)
- [Render Model Hooks](../reference/render-model-hooks.md)
- [Customer Rust vs third-party WASM](../reference/customer-vs-wasm.md)
