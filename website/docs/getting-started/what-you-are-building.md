---
title: What You Are Building
---

This tutorial builds one customer-owned application called `tutorial-app`.

The finished app is intentionally broad enough to exercise the main Coil seams in one codebase:

- a customer root workspace
- a runtime composed from official modules plus customer Rust
- a shared theme and template shell
- multiple sites and locales
- CMS pages, reusable blocks, memberships, discovery, and event-led pages

## Purpose

Before you generate files, fix the shape of the app in concrete terms:

- `Cargo.toml` owns the customer workspace
- `app.toml` owns product structure
- `platform.dev.toml` owns local runtime behavior
- `crates/tutorial-app-app` owns application composition
- `crates/tutorial-app-backend` owns customer-specific backend behavior
- `crates/tutorial-app-bin` owns process startup
- `templates/` owns HTML
- `theme/assets/` owns CSS and static assets
- `content/` owns editorial records introduced later

The early chapters build those seams in that order.

## Target Project Layout

The first half of the tutorial builds toward this tree:

```text
tutorial-app/
  Cargo.toml
  app.toml
  platform.dev.toml
  docker-compose.yml
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
  theme/
    assets/
      site.css
```

Later chapters add:

```text
  content/
    pages/
    shared-blocks/
  templates/
    blocks/
  crates/
    tutorial-app-backend/
      src/lib.rs
```

Those later additions matter because the tutorial eventually moves from a static storefront shell to
structured editorial content, reusable blocks, memberships, and dynamic routes.

## The Product Manifest

The product starts in `app.toml`. This file describes the app you are building, not the machine
you run it on.

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

What each section does:

- `name` and `display_name`
  Give the app a stable id and a human-readable label.
- `[i18n]`
  Declares the app-wide locale set. Later site definitions narrow that set per site.
- `[[sites]]`
  Declares the sites the app serves, which domains belong to each site, and which locales each site
  supports.
- `[theme]`
  Tells the asset pipeline where customer-owned styles and static assets live.
- `[auth]`
  Names the auth package the runtime should load.
- `[[modules]]`
  Decides which official module surfaces exist in the app at all.

What you will edit later:

- add more sites or locales
- add more official modules
- point the app at different theme asset roots

What this file enables:

- site-aware routing
- locale-aware URLs
- module composition
- theme asset resolution

## The Composition Root

The app crate turns the manifest and the linked backend into a running runtime.

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

What each section does:

- `PlatformConfig::from_file("platform.dev.toml")`
  Validates the local runtime configuration before you try to boot the server.
- `.with_customer_plugin(...)`
  Links customer-owned Rust into the runtime.
- `.with_module(...)`
  Links official modules into the runtime.
- `.run_from_env()`
  Starts the server using the configured app plus environment-provided secrets.

What you will edit later:

- add more official modules
- register more customer-owned backend behavior
- extend the small command surface if the app needs more developer commands

What this file enables:

- one visible composition point for the whole customer app
- a clean split between official modules and customer behavior
- a customer-owned binary that stays thin

## The Base Shell

The tutorial starts with a simple shared layout and one home page so later CMS pages, account
pages, and storefront routes all have somewhere consistent to render.

`templates/layouts/base.html`:

```html
<!doctype html>
<html xmlns:coil="https://coil.rs" lang="en-GB" coil:attr="lang=${locale}">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title coil:text="${page.title}">Tutorial App</title>
    <link rel="stylesheet" href="/theme/assets/site.css" coil:href="asset('theme/assets/site.css')" />
  </head>
  <body>
    <a class="skip-link" href="#main">Skip to content</a>
    <header class="site-header">
      <a class="brand" href="/" coil:attr="href=${links.home}">Tutorial App</a>
      <nav aria-label="Primary">
        <a href="/" coil:attr="href=${links.home}">Home</a>
        <a href="/shop" coil:attr="href=${links.catalog}">Shop</a>
        <a href="/account" coil:attr="href=${links.account}">Account</a>
      </nav>
    </header>
    <main id="main" class="site-main">
      <coil:block coil:insert="${content}">
        <p>Page content</p>
      </coil:block>
    </main>
    <footer class="site-footer">
      <small>Tutorial App</small>
    </footer>
  </body>
</html>
```

`templates/pages/home.html`:

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
      <div class="hero-actions">
        <a class="button" href="/shop">Shop now</a>
        <a class="button button--secondary" href="/pages/membership">Memberships</a>
      </div>
    </section>
  </body>
</html>
```

What these files do:

- the layout owns the header, footer, page title, and stylesheet link
- the home page owns the first public route content
- both files already use runtime-provided values such as `page.title`, `locale`, and `links.home`

What you will edit later:

- add site and locale switching to the layout
- add CMS-backed sections to the home page
- add account and storefront entry points as the app grows

What these files enable:

- one shared shell for every page
- route-aware navigation instead of hard-coded final URLs
- a stable home for later CMS and membership work

## The Early Build Sequence

The first five chapters establish these concrete states:

1. a customer workspace boots through its own binary
2. each major file has one clear responsibility
3. the app renders a real shared shell instead of a placeholder page
4. the app answers multiple hosts and locales
5. the app moves from flat pages toward structured editorial content

## Runnable Checkpoint

After chapter 5, you should be able to run:

```bash
docker compose up -d
cargo run -p tutorial-app-bin -- validate
cargo run -p tutorial-app-bin -- serve
```

And answer these questions from the code in front of you:

- which file defines product structure
- which file defines local runtime behavior
- which crate composes the runtime
- which crate owns customer-specific backend rules
- which files own HTML
- which files own CSS

## What To Read Next

- [Create the Project](../create-the-project/)
