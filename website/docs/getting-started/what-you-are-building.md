---
title: What You Are Building
---

Before generating a project, anchor the product shape.

This tutorial is not building a toy storefront. It is building a customer app that forces the main
Coil seams to show up in realistic places.

## The Target App

The tutorial app is a small but credible customer product with these responsibilities:

- retail merchandising
- event discovery and bookings
- memberships and audience-aware pages
- editorial landing pages built from structured blocks
- multiple sites and locales
- at least one linked customer Rust rule

That product shape is deliberate. A smaller demo would hide too much of Coil's composition model.

## The Concrete End State

By the time you finish the tutorial, the app root should look roughly like this:

```text
tutorial-app/
  Cargo.toml
  app.toml
  platform.dev.toml
  docker-compose.yml
  content/
    pages/
      spring-sale.json
    shared-blocks/
      delivery-promises.json
  crates/
    tutorial-app-app/
      src/lib.rs
    tutorial-app-backend/
      src/lib.rs
    tutorial-app-bin/
      src/main.rs
  templates/
    layouts/
      base.html
    pages/
      home.html
    blocks/
      featured-events.html
  theme/
    assets/
      site.css
```

Those are not illustrative placeholders. They are the files the rest of the tutorial writes.

## What The App Should Already Say In Code

The root manifest should read like a customer app, not a generic example:

```toml
name = "tutorial-app"
display_name = "Tutorial App"

[i18n]
default_locale = "en-GB"
supported_locales = ["en-GB", "fr-FR"]

[[sites]]
id = "tutorial-uk"
display_name = "Tutorial UK"
brand_name = "Tutorial"
canonical_domain = "uk.127.0.0.1.nip.io"
additional_domains = ["www.127.0.0.1.nip.io"]
default_locale = "en-GB"
supported_locales = ["en-GB"]

[[sites]]
id = "tutorial-fr"
display_name = "Tutorial France"
brand_name = "Tutorial"
canonical_domain = "fr.127.0.0.1.nip.io"
additional_domains = []
default_locale = "fr-FR"
supported_locales = ["fr-FR"]
```

The app crate should already read like visible runtime composition:

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

The public shell should already look like a product, not a framework starter:

```html
<!doctype html>
<html xmlns:coil="https://coil.rs" coil:replace="~{layouts/base}">
  <body>
    <section class="hero">
      <p class="eyebrow">New season</p>
      <h1>Retail, memberships, and editorial content in one customer app.</h1>
      <p>
        Start with a branded shell now so later CMS pages, commerce flows, and reusable blocks have
        a real home.
      </p>
    </section>
  </body>
</html>
```

## The Checkpoints You Are Actually Building

The tutorial proceeds through these concrete checkpoints:

### Checkpoint 1: the generated app boots

You create the project, run local dependencies, and validate that the customer binary owns the
application lifecycle.

### Checkpoint 2: the runtime shape makes sense

You can point at `app.toml`, `platform.dev.toml`, the binary crate, the app crate, and the backend
crate and explain which one owns which concern.

### Checkpoint 3: the base shell is branded

You have real checked-in files for:

- `templates/layouts/base.html`
- `templates/pages/home.html`
- `theme/assets/site.css`

### Checkpoint 4: sites and locales are visible

You replace flat single-site config with real multi-site files:

- `app.toml`
- `platform.dev.toml`
- `templates/layouts/base.html`

### Checkpoint 5: the editorial model is structured

You add:

- `content/pages/spring-sale.json`
- content model declarations in `app.toml`

### Checkpoint 6: pages are block-composed

You add:

- `content/shared-blocks/delivery-promises.json`
- block schema declarations in `app.toml`
- block instances in `content/pages/spring-sale.json`

### Checkpoint 7: one block is dynamic

You connect:

- `content/pages/spring-sale.json`
- `crates/tutorial-app-backend/src/lib.rs`
- `templates/blocks/featured-events.html`

## What You Will Not Build

The tutorial is still bounded.

It will not attempt to prove:

- a complete ERP
- a full custom payment processor
- a full search cluster implementation
- a fake "everything platform" in one chapter

The goal is to make Coil's actual joints visible, not to drown the reader in fake complexity.

## Where Reference Docs Fit

You do not need to stop reading the tutorial to inspect the underlying contracts.

For example:

- when you want the exact `app.toml` keys, use [App TOML](../reference/app-toml.md)
- when you want exact runtime config sections, use [Platform Config](../reference/platform-config.md)
- when you want the long-term customization boundary, use [Linked Rust hook APIs](../reference/linked-rust-hook-apis.md)

The tutorial gives the sequence. Reference docs give the exact lookup detail.

## What To Read Next

- [Create the Project](create-the-project.md)
