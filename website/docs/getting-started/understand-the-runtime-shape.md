---
title: Understand the Runtime Shape
---

This chapter walks the generated project in the same order the runtime uses it.

## Purpose

At this point you already have a bootable project. The next job is to separate responsibilities so
later chapters do not mix product decisions with local runtime setup or frontend build concerns:

- product structure
- local runtime configuration
- Rust runtime composition
- template rendering
- frontend source files
- compiled asset output

Keep this split in mind as you read the files:

- `app.toml` defines the product
- `platform.dev.toml` defines local runtime behavior
- `tutorial-app-app` composes the runtime
- `tutorial-app-bin` starts the process
- `tutorial-app-backend` owns customer-specific backend behavior
- `theme/frontend/*` owns frontend source files
- `theme/assets/*` is compiled output loaded by templates

## `app.toml`

`app.toml` is the product manifest. It describes the app Coil should serve, regardless of where you run it.

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

What each section controls:

- `[domains]`
  Declares the public hosts that belong to the app.
- `[i18n]`
  Declares the locale contract the router and template model rely on.
- `[theme]`
  Points at compiled assets. This is why the build script writes into `theme/assets`.
- `[auth]`
  Names the auth package that contributes login/session behavior.
- `[[modules]]`
  Chooses which first-party modules are part of this app.

What this file does not do:

- it does not point templates at individual source CSS or TypeScript files
- it does not define local ports or database URLs

## `platform.dev.toml`

`platform.dev.toml` is the local runtime config. It describes development wiring, not the product itself.

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

What each section controls:

- `[server]`
  Picks the local bind address.
- `[database]`, `[cache]`, `[jobs]`
  Pick the local backends used in development.
- `[storage]`
  Picks the path for local file-backed runtime state.

What this file does not do:

- it does not decide which frontend bundle a page loads
- it does not declare source asset entrypoints

## `apps/shoppr/package.json`

`package.json` declares the frontend toolchain and the commands that turn source bundles into publishable assets.

```json title="apps/shoppr/package.json"
{
  "name": "shoppr-frontend",
  "private": true,
  "type": "module",
  "scripts": {
    "build": "node ./theme/build/build.mjs",
    "watch": "node ./theme/build/build.mjs --watch"
  },
  "dependencies": {
    "@hotwired/stimulus": "^3.2.2",
    "@hotwired/turbo": "^8.0.12"
  },
  "devDependencies": {
    "autoprefixer": "^10.4.21",
    "esbuild": "^0.25.0",
    "postcss": "^8.5.3",
    "postcss-import": "^16.1.0",
    "postcss-nesting": "^13.0.2"
  }
}
```

What each section controls:

- `scripts.build`
  Produces the assets that templates will load in production.
- `scripts.watch`
  Rebuilds while you edit source files during development.
- `dependencies`
  Ship to the browser.
- `devDependencies`
  Stay in the build toolchain.

## `apps/shoppr/theme/build/build.mjs`

`theme/build/build.mjs` is the bridge between source files and loadable theme assets.

```js title="apps/shoppr/theme/build/build.mjs"
import { mkdir, readFile, writeFile } from "node:fs/promises";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";
import esbuild from "esbuild";
import postcss from "postcss";
import postcssImport from "postcss-import";
import postcssNesting from "postcss-nesting";
import autoprefixer from "autoprefixer";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const appRoot = path.resolve(__dirname, "..", "..");
const frontendRoot = path.join(appRoot, "theme", "frontend");
const assetRoot = path.join(appRoot, "theme", "assets");
const watchMode = process.argv.includes("--watch");

const jsEntries = {
  site: path.join(frontendRoot, "site.ts"),
  admin: path.join(frontendRoot, "admin.ts"),
  "cms-editor": path.join(frontendRoot, "cms-editor.ts")
};

const cssEntries = {
  site: path.join(frontendRoot, "site.css"),
  admin: path.join(frontendRoot, "admin.css"),
  "cms-editor": path.join(frontendRoot, "cms-editor.css")
};

async function buildCss() {
  const processor = postcss([postcssImport(), postcssNesting(), autoprefixer()]);
  await mkdir(assetRoot, { recursive: true });

  for (const [name, sourcePath] of Object.entries(cssEntries)) {
    const source = await readFile(sourcePath, "utf8");
    const result = await processor.process(source, {
      from: sourcePath,
      to: path.join(assetRoot, `${name}.css`)
    });
    await writeFile(path.join(assetRoot, `${name}.css`), result.css, "utf8");
  }
}

async function buildJs() {
  await mkdir(assetRoot, { recursive: true });
  return esbuild.build({
    entryPoints: jsEntries,
    outdir: assetRoot,
    bundle: true,
    format: "iife",
    target: "es2020",
    platform: "browser",
    sourcemap: false,
    logLevel: "info"
  });
}

async function buildAll() {
  await Promise.all([buildCss(), buildJs()]);
}

async function watch() {
  await buildCss();

  const context = await esbuild.context({
    entryPoints: jsEntries,
    outdir: assetRoot,
    bundle: true,
    format: "iife",
    target: "es2020",
    platform: "browser",
    sourcemap: "inline",
    logLevel: "info"
  });

  await context.watch();

  let cssTimer = null;
  fs.watch(frontendRoot, { recursive: true }, (_eventType, filename) => {
    if (!filename || (!filename.endsWith(".css") && !filename.endsWith(".ts"))) {
      return;
    }
    if (cssTimer) {
      clearTimeout(cssTimer);
    }
    cssTimer = setTimeout(async () => {
      try {
        await buildCss();
        console.log("[shoppr-frontend] CSS rebuilt");
      } catch (error) {
        console.error("[shoppr-frontend] CSS build failed");
        console.error(error);
      }
    }, 75);
  });

  console.log("[shoppr-frontend] watching theme/frontend");
}

if (watchMode) {
  await watch();
} else {
  await buildAll();
}
```

The important boundaries are:

- `theme/frontend/*` is source
- `theme/assets/*` is compiled output
- `site`, `admin`, and `cms-editor` are separate surfaces

## `crates/tutorial-app-app/src/lib.rs`

The app crate turns the manifest, the config, the modules, and the backend plugin into a running
runtime.

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

This file composes the app, but it does not bundle frontend code. The frontend asset boundary is
already settled before the runtime serves templates.

## `crates/tutorial-app-bin/src/main.rs`

The binary crate owns process startup:

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

## `crates/tutorial-app-backend/src/lib.rs`

The backend crate is where customer-owned runtime behavior starts:

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

This file shapes server-side behavior. It does not replace the frontend build pipeline.

## The Compiled Asset Contract

Templates load compiled assets from `theme/assets`, not source files from `theme/frontend`.

The real Shoppr storefront shell looks like this:

```html title="apps/shoppr/templates/layouts/base.html"
<!doctype html>
<html
  xmlns:coil="https://coil.rs"
  coil:fragment="shell"
  lang="en-GB"
  coil:attr="lang=${locale}"
>
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title coil:text="${page_title}">Shoppr</title>
    <link rel="preconnect" href="https://fonts.googleapis.com" />
    <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin="anonymous" />
    <link
      href="https://fonts.googleapis.com/css2?family=Manrope:wght@400;500;600;700;800&amp;family=Prata&amp;display=swap"
      rel="stylesheet"
    />
    <link rel="stylesheet" href="/theme/assets/site.css" coil:href="asset('theme/assets/site.css')" />
    <script src="/theme/assets/site.js" coil:src="asset('theme/assets/site.js')" defer="defer"></script>
  </head>
  <body class="shoppr-shell">
    <header class="site-header">
      <div class="site-header__utility">
        <p>Now serving UK, France, and Poland with route-level locale switching.</p>
      </div>
    </header>
    <main class="site-main" coil:slot="content">
      <section>Content goes here.</section>
    </main>
  </body>
</html>
```

The real Shoppr admin dashboard loads a different bundle:

```html title="apps/shoppr/templates/admin/dashboard.html"
<!doctype html>
<html
  xmlns:coil="https://coil.rs"
  coil:with="page_title='Shoppr Admin'"
  lang="en-GB"
  coil:attr="lang=${locale}"
>
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title coil:text="${page_title}">Shoppr Admin</title>
    <link rel="stylesheet" href="/theme/assets/admin.css" coil:href="asset('theme/assets/admin.css')" />
    <script src="/theme/assets/admin.js" coil:src="asset('theme/assets/admin.js')" defer="defer"></script>
  </head>
  <body class="harbor harbor--admin">
    <header class="site-header">
      <a href="/" class="brand">Shoppr</a>
      <nav coil:replace="~{admin/nav}"></nav>
    </header>
    <main class="site-main admin-page">
      <section class="admin-page__intro">
        <p class="admin-page__eyebrow">Operator dashboard</p>
        <h1 coil:text="${page.title}">Shoppr Admin</h1>
      </section>
    </main>
  </body>
</html>
```

The real Shoppr CMS editor surface loads a third bundle:

```html title="apps/shoppr/templates/cms/pages.html"
<!doctype html>
<html
  xmlns:coil="https://coil.rs"
  coil:with="page_title='Pages'"
  lang="en-GB"
  coil:attr="lang=${locale}"
>
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title coil:text="${page_title}">Pages</title>
    <link rel="stylesheet" href="/theme/assets/cms-editor.css" coil:href="asset('theme/assets/cms-editor.css')" />
    <script src="/theme/assets/cms-editor.js" coil:src="asset('theme/assets/cms-editor.js')" defer="defer"></script>
  </head>
  <body class="harbor harbor--admin">
    <header class="site-header">
      <a href="/" class="brand">Shoppr</a>
      <nav coil:replace="~{admin/nav}"></nav>
    </header>
    <main class="site-main admin-page">
      <section class="admin-page__intro">
        <p class="admin-page__eyebrow">Content operations</p>
        <h1 coil:text="${page.title}">Pages</h1>
      </section>
    </main>
  </body>
</html>
```

What these templates prove:

- storefront pages load `site.*`
- admin pages load `admin.*`
- CMS editor pages load `cms-editor.*`
- templates never import `theme/frontend/*.ts` or `theme/frontend/*.css` directly

## Runnable Checkpoint

Use the real Shoppr frontend and runtime commands:

```bash
cd apps/shoppr
npm install
npm run build
./scripts/prepare-local-dev.sh
cargo run -p shoppr -- validate
COIL_COOKIE_SECRET=01234567012345670123456701234567 \
COIL_CSRF_SECRET=76543210765432107654321076543210 \
cargo run -p shoppr -- up --config platform.dev.toml
```

At this point you should be able to answer:

- which file owns frontend source
- which file owns compiled asset output
- which file defines the asset build graph
- which templates load storefront, admin, and CMS editor bundles

## What To Read Next

- [Build the Base Theme](../build-the-base-theme/)
