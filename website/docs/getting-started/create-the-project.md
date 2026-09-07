---
title: Create the Project
---

This chapter creates the customer workspace and maps it to the real Shoppr frontend scaffold that
already exists in this repository.

## Purpose

At the end of this chapter you will have:

- a customer-owned Cargo workspace
- a small binary crate
- an app crate that composes the runtime
- a backend crate reserved for customer-specific behavior
- a product manifest
- a local runtime config
- a frontend toolchain with separate storefront, admin, and CMS editor entrypoints
- local infrastructure for Postgres and Redis

## Generate the Workspace

Run:

```bash
cargo install cargo-coil --locked
cargo coil new tutorial-app
cd tutorial-app
```

The generated project should contain a workspace root plus three crates:

- `tutorial-app-bin` for process startup
- `tutorial-app-app` for runtime composition
- `tutorial-app-backend` for customer-owned behavior

## Root `Cargo.toml`

`Cargo.toml` defines the workspace boundary and the shared Rust dependency set.

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

What each section does:

- `[workspace].members`
  Declares the three crates that make up the customer app.
- `resolver = "2"`
  Uses Cargo's modern feature resolver.
- `[workspace.package]`
  Sets defaults inherited by member crates.
- `[workspace.dependencies]`
  Centralizes shared Rust dependencies.

## `apps/shoppr/package.json`

The checked-in frontend toolchain sits at the app root, alongside the Rust workspace:

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

What each section does:

- `scripts.build`
  Runs the real Shoppr asset build.
- `scripts.watch`
  Rebuilds Shoppr assets while you edit source files.
- `dependencies`
  Pull in the runtime browser libraries Shoppr actually uses.
- `devDependencies`
  Pull in the build tools Shoppr actually uses.

What you should edit:

- add more frontend dependencies only when the app really needs them
- keep the entrypoint contract stable so templates can keep loading the same logical bundles

## `apps/shoppr/theme/build/build.mjs`

This file turns Shoppr frontend source files into compiled theme assets:

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

What each section does:

- `frontendRoot`
  Declares `theme/frontend` as the source tree you edit.
- `assetRoot`
  Declares `theme/assets` as build output.
- `jsEntries`
  Produces the three real Shoppr JavaScript bundles:
  - `site.js` for the storefront
  - `admin.js` for admin pages
  - `cms-editor.js` for the CMS page editor
- `cssEntries`
  Produces the matching CSS bundles.
- `buildCss()`
  Runs PostCSS over source CSS files.
- `buildJs()`
  Bundles TypeScript with esbuild.
- `watch()`
  Keeps rebuilding during local development.

What you should edit:

- add entrypoints only when the app genuinely needs another distinct surface
- do not point templates at source files in `theme/frontend`

## `crates/tutorial-app-bin/src/main.rs`

The binary crate owns process startup and exit behavior:

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

What each section does:

- calls the app crate
- prints failures to stderr
- returns a shell-friendly exit code

## `crates/tutorial-app-app/src/lib.rs`

The app crate owns runtime composition:

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

- `validate`
  Checks local runtime config.
- `serve`
  Boots the customer app.
- `.with_customer_plugin(...)`
  Links customer Rust into the runtime.
- `.with_module(...)`
  Links official modules into the runtime.

## `crates/tutorial-app-backend/src/lib.rs`

The backend crate is where customer-specific runtime behavior will live:

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

What each section does:

- declares the customer-owned plugin type
- reserves the hook-registration seam for later chapters

## `app.toml`

`app.toml` defines the product structure:

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

The important frontend section is `[theme]`: it points the runtime at compiled assets under
`theme/assets`, not the source files under `theme/frontend`.

## `platform.dev.toml`

`platform.dev.toml` defines how the app runs locally:

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

This file does not choose frontend bundles directly. It chooses the local runtime environment that
serves the app and its compiled assets.

## `docker-compose.yml`

`docker-compose.yml` starts the local services the runtime config expects:

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

## Runnable Checkpoint

Use the real Shoppr commands:

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

What should happen:

- `cd apps/shoppr` enters the real customer workspace
- `npm install` installs Turbo, Stimulus, esbuild, and PostCSS
- `npm run build` emits `theme/assets/site.*`, `theme/assets/admin.*`, and
  `theme/assets/cms-editor.*`
- `./scripts/prepare-local-dev.sh` prepares the local Cargo overlay used by the checked-in app
- `validate` loads Shoppr's `platform.dev.toml`
- `up --config platform.dev.toml` boots the app through the real Shoppr binary

## What To Read Next

- [Understand the Runtime Shape](../understand-the-runtime-shape/)
