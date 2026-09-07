---
title: Getting Started Tutorial
---

This tutorial builds a customer-owned application named `tutorial-app`, but the concrete frontend
reference in this repository is the checked-in Shoppr workspace under `apps/shoppr`.

Shoppr is the example to copy because it already uses the frontend split this tutorial is teaching:

- server-rendered HTML and fragments
- Turbo for HTML-over-the-wire navigation and form enhancement
- Stimulus for browser-side controllers
- PostCSS for CSS compilation
- esbuild for JavaScript bundling
- compiled assets emitted into `theme/assets`

## The First Working State

The real checked-in frontend root looks like this:

```text
apps/shoppr/
  Cargo.toml
  app.toml
  platform.dev.toml
  docker-compose.yml
  package.json
  crates/
    shoppr-app/
    shoppr-backend/
    shoppr-bin/
  templates/
  theme/
    build/
      build.mjs
    frontend/
      site.ts
      site.css
      admin.ts
      admin.css
      cms-editor.ts
      cms-editor.css
    assets/
  auth/shoppr-auth/
```

Those files are the pieces you need to keep straight from the start:

- `Cargo.toml` groups the Rust crates into one customer workspace
- `app.toml` defines the product contract Coil boots
- `platform.dev.toml` defines how that product runs locally
- `package.json` is the browser build entrypoint, not a sidecar afterthought
- `theme/build/build.mjs` turns source bundles into theme assets that templates can load
- `theme/frontend/*` is the editable source tree for CSS and TypeScript
- `theme/assets/*` is compiled output that Coil publishes and templates reference
- `crates/shoppr-app` composes the runtime and module graph
- `crates/shoppr-backend` is where customer-owned backend logic lives
- `crates/shoppr-bin` is the process entrypoint

## The First Five Files

### `Cargo.toml`

The root workspace file answers one question: which Rust crates make up the customer app?

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

What you are looking at:

- `[workspace].members`
  Lists the three crates Cargo builds together.
- `[workspace.package]`
  Sets package defaults so the member crates do not each repeat edition, version, and license.
- `[workspace.dependencies]`
  Centralizes the shared dependencies the three crates use together.

### `app.toml`

`app.toml` is the product contract. It answers: what app is Coil booting?

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
  Defines the hostnames the app serves.
- `[i18n]`
  Defines the locale contract the runtime and router use.
- `[theme]`
  Points Coil at the compiled asset directory, not the source CSS or TypeScript.
- `[auth]`
  Tells the runtime which auth package directory to load.
- `[[modules]]`
  Chooses which first-party capabilities are installed into this app.

### `platform.dev.toml`

`platform.dev.toml` is the local runtime contract. It answers: how should this app run on your machine?

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
  Picks the local bind address for the HTTP server.
- `[database]`, `[cache]`, `[jobs]`
  Pick the local backends the runtime will connect to.
- `[storage]`
  Sets the local path for file-backed runtime state.

### `apps/shoppr/package.json`

The checked-in Shoppr frontend toolchain also lives at the app root. That is deliberate. The
browser build is part of the customer app, not an unrelated subproject.

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

What this file controls:

- `scripts.build`
  Runs the production asset build and writes into `theme/assets/`.
- `scripts.watch`
  Rebuilds while you edit `theme/frontend/*`.
- `dependencies`
  Provide the runtime browser libraries that execute in the page.
- `devDependencies`
  Provide the local toolchain that compiles the bundles.

### `apps/shoppr/theme/build/build.mjs`

The checked-in Shoppr build script defines the asset contract:

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

Important sections:

- `jsEntries`
  Defines the three real browser bundles used by Shoppr.
- `cssEntries`
  Defines matching stylesheet entrypoints.
- `assetRoot`
  Makes `theme/assets` compiled output.
- `buildCss()` and `buildJs()`
  Compile source files from `theme/frontend/*`.
- `watch()`
  Rebuilds during local editing.

## The Working Loop

For the real frontend build loop in this repo, use the Shoppr workspace directly:

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

Those commands exercise different parts of the system:

- `cd apps/shoppr`
  Puts you in the real checked-in customer workspace.
- `npm install`
  Installs the frontend toolchain and browser dependencies.
- `npm run build`
  Compiles `theme/frontend/*` into `theme/assets/*`.
- `./scripts/prepare-local-dev.sh`
  Writes the local Cargo patch overlay used for in-repo development.
- `validate`
  Proves the Shoppr workspace and config agree.
- `up --config platform.dev.toml`
  Boots the real Shoppr app through the customer-owned binary.

During theme work, add:

```bash
cd apps/shoppr
npm run watch
```

That command keeps rebuilding the storefront, admin, and CMS editor bundles while you edit source
files under `theme/frontend/`.

## Read Order

Read the tutorial in this order:

1. [What You Are Building](../what-you-are-building/)
2. [Create the Project](../create-the-project/)
3. [Understand the Runtime Shape](../understand-the-runtime-shape/)
4. [Build the Base Theme](../build-the-base-theme/)
5. [Add Sites, Markets, and Locales](../add-sites-markets-and-locales/)
6. [Add a Real Content Model](../add-a-real-content-model/)
7. [Build Reusable Blocks](../build-reusable-blocks/)
8. [Add Dynamic Blocks](../add-dynamic-blocks/)
9. [Model Brands, Categories, and Discovery](../model-brands-categories-and-discovery/)
10. [Add Authentication and Customer Accounts](../add-authentication-and-customer-accounts/)
11. [Add Memberships and Audience Gating](../add-memberships-and-audience-gating/)
12. [Add Events and Timeslots](../add-events-and-timeslots/)
13. [Add Bookings, Reservations, and Validation](../add-bookings-reservations-and-validation/)
14. [Add Passes or Credits](../add-passes-or-credits/)
15. [Add Admin Resources](../add-admin-resources/)
16. [Add One Reproducible Integration](../add-one-reproducible-integration/)
17. [Add Jobs, Notifications, and Scheduled Work](../add-jobs-notifications-and-scheduled-work/)
18. [Add Observability and Troubleshooting](../add-observability-and-troubleshooting/)
19. [Prepare for Production](../prepare-for-production/)
20. [Where to Go Next](../where-to-go-next/)

Supporting pages:

- [Customer Project Layout](../customer-project-layout/)
- [Linked Rust Backends](../linked-rust-backends/)

## Checkpoint

From the checked-in Shoppr workspace, these commands should all succeed:

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

At this point the tutorial has a concrete Shoppr-based starting state for both Rust and frontend
work.
