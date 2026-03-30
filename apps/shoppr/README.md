# Shoppr

Shoppr is the reference Coil customer app in this repository.

It is not the platform itself. It is an example deployable product built on top of Coil:

- it selects official modules
- it provides the customer app manifest and platform config
- it owns the storefront templates and theme assets
- it carries customer-specific auth bindings
- it is the fastest way to see what a real Coil app looks like

If you are new to the repo, start here.

For local development, Shoppr is intentionally turnkey for the full public and authenticated
journey without requiring a pre-seeded database user or live Stripe credentials:

- authenticated routes are bootstrapped through the built-in `__dev` login shortcuts
- checkout uses the built-in local hosted-checkout stub until you supply real Stripe test keys
- the default stack is meant to let a new developer browse, sign in, add to cart, check out, and inspect admin surfaces on first run

## What Is In This Folder

`app.toml`
- the customer app manifest
- selects the app identity, locales, theme, auth package, and installed modules

`platform.dev.toml`
- the local development platform config used by Docker Compose
- points at Postgres, Redis, MinIO, and local dev-safe HTTP settings

`platform.toml`
- the production-oriented platform config

`auth/shoppr-auth/`
- the customer app auth package
- this is where app-specific capability mappings live

`content/`
- customer-owned content model definitions such as page types

`templates/`
- the HTML-first template tree for storefront, account, CMS, and admin pages

`theme/`
- the Shoppr theme assets
- CSS, images, and other published frontend assets live here

`extensions/`
- the deployment target for customer-specific WASM extension artifacts

`backend/`
- customer-app-owned Rust backend example code
- the linked customer-backend implementation lives in this tree and is registered through the Shoppr workspace crates
- the `backend/shoppr-loyalty-backend` crate is the shared business-rules library the linked crate wraps

`Cargo.toml`, `Cargo.lock`, `crates/`
- the Shoppr nested Cargo workspace
- `crates/shoppr-bin` is the customer binary composition root
- `crates/shoppr-app` owns manifest/config/auth loading plus Shoppr runtime composition
- `crates/shoppr-backend` is the linked customer Rust backend registered into the customer binary

`docker/`, `Dockerfile`, `docker-compose.yml`
- the local developer stack

## What Shoppr Demonstrates

Shoppr is meant to show the boundary described in `docs/design`:

- Coil core provides the runtime, routing, storage, cache, auth execution, jobs, TLS, and asset publication
- official modules provide reusable product batteries like CMS, commerce, memberships, events, admin, and ops
- Shoppr provides composition, branding, templates, app policy, and customer-specific behaviour

That is why this folder matters. It is the customer-app layer, not a random demo directory.

## Shoppr As A Customer-Root Workspace

Shoppr now has its own nested Cargo workspace in this folder.

That is the chapter 96 shape in repo form:

- Coil is modeled here as normal upstream `0.1.0` dependencies
- Shoppr owns a binary crate that links the official modules it needs
- Shoppr owns a linked backend crate that registers customer-specific behaviour through public Coil APIs
- the checked-in backend library remains customer-owned code, but the store consumes it through the linked plugin path rather than a separate service boundary

The optional sidecar adapter still exists for integrations that genuinely need a separate
HTTP/process boundary, but it is not the primary Shoppr customization model.

The committed workspace is intentionally free of `patch.crates-io` overlays and other repo-local
dependency rewrites. Shoppr is checked in as a normal customer project that can resolve
Coil from an upstream registry or pinned git source.

The Docker story now follows the same split:

- `docker compose up --build` uses `apps/shoppr` as the Docker build context and expects
  Coil crates plus the Shoppr customer binary to resolve like normal upstream dependencies
- `docker compose -f docker-compose.yml -f docker-compose.repo.yml up --build` is the explicit
  repo-maintainer override when you are building Shoppr against this monorepo before those
  upstream packages are published

If you are iterating on Shoppr from inside the Coil repository before those upstream crates
are published, keep that override local and uncommitted. The supported maintainer path is:

```bash
./scripts/prepare-local-dev.sh
```

That writes `apps/shoppr/.cargo/config.toml` with repo-local path patches. The file is
ignored by git and exists only so this checked-in example can build against the current Coil
workspace without polluting the committed customer manifest.

From `apps/shoppr`:

```bash
./scripts/prepare-local-dev.sh
cargo run -p shoppr -- describe
cargo run -p shoppr -- validate
cargo run -p shoppr -- migrate apply --dry-run
cargo run -p shoppr -- assets publish
```

That prints the active app root, config, linked modules, and the linked customer plugin ids added
by the Shoppr workspace.

The checked-in linked plugin in this workspace is:

- `Shoppr Linked Backend` (`shoppr-backend`)

Shoppr also demonstrates three sites under one customer app boundary:

- `shoppr-uk`
  - flagship UK storefront
  - host: `uk.localhost`
  - default locale: `en-GB`
- `shoppr-fr`
  - French editorial storefront
  - host: `fr.localhost`
  - default locale: `fr-FR`
- `shoppr-pl`
  - Polish assortment with localized merchandising
  - host: `pl.localhost`
  - default locale: `pl-PL`

Those `*.localhost` hosts resolve locally without external DNS or `/etc/hosts` edits, so the
three-site demo stays self-contained.

Once Shoppr is running, the same linked-backend shape is visible inside the app itself:

- `/admin` now renders the linked customer plugin metadata from the runtime plan itself
- `cargo run -p shoppr -- describe` prints the linked plugin ids in the customer workspace
- `cargo run -p shoppr -- linked-backend demo` executes the checked-in linked backend rules directly from the customer workspace without the optional sidecar

From `apps/shoppr`, the shortest concrete linked-backend walkthrough is:

```bash
./scripts/prepare-local-dev.sh
cargo run -p shoppr -- linked-backend describe
cargo run -p shoppr -- linked-backend demo
```

That path stays entirely inside the customer-root workspace. It exercises the exact linked Rust
crate Shoppr compiles into the app and prints real loyalty-preview, checkout-review, and
CRM-contact routing outputs from the checked-in sample requests.

## Prerequisites

You need:

- Docker Desktop or Docker Engine plus the Compose plugin
- a working `docker compose` command
- optionally, Stripe CLI if you want to test real local webhook forwarding

You do not need to install Postgres, Redis, or MinIO manually for the default local run.

## Quick Start

From this folder:

```bash
cp .env.example .env
./scripts/prepare-local-dev.sh
cargo coil dev
```

That is the managed customer-app path. `cargo coil dev` uses the checked-in app root, starts
Postgres and Redis from the local `docker-compose.yml`, injects the standard development
environment variables, and runs the `shoppr` customer binary from this nested workspace.

If you want the full Docker stack instead of the managed host loop, you can still run:

```bash
cp .env.example .env
docker compose up --build
```

If you are a Coil maintainer building Shoppr from inside this repository before upstream
crate publication, use the explicit repo override:

```bash
cp .env.example .env
docker compose -f docker-compose.yml -f docker-compose.repo.yml up --build
```

Then open:

- `http://uk.localhost:8080/`
- `http://uk.localhost:8080/__dev`

If you want to exercise the three-site demo explicitly, open these real local hosts:

- `http://uk.localhost:8080/en-GB/shop`
- `http://fr.localhost:8080/fr-FR/shop`
- `http://pl.localhost:8080/pl-PL/shop/products/harbor-scarf`

The `__dev` page gives you one-click local login shortcuts for the checked-in customer and admin
paths. This is the local bootstrap mechanism for authenticated walkthroughs; Shoppr does not
depend on a seeded admin password for first-run development.

## What To Expect During Startup

These MinIO lines are expected:

```text
Added `local` successfully.
Bucket created successfully `local/shoppr`.
Access permission for `local/shoppr` is set to `download`
```

That is the bootstrap job creating the local object-store bucket and making published theme assets downloadable in the dev stack.

The `app` container now runs Shoppr's own lifecycle command:

1. Shoppr builds and validates the customer runtime from `platform.dev.toml`
2. Shoppr applies pending executable migrations through the customer binary
3. Shoppr publishes theme assets through the same customer runtime build path
4. Shoppr starts the storefront/admin server

You can run the same end-to-end lifecycle directly from the customer workspace:

```bash
./scripts/prepare-local-dev.sh
cargo coil dev
```

The committed Cargo workspace stays upstream-clean. The default checked-in `docker compose up`
path now uses only the Shoppr folder as its Docker build context. The separate
`docker-compose.repo.yml` override is the only in-repo maintainer convenience path, and it is
explicit about using the Coil monorepo plus local Cargo patching before the `coil-*` crates
are published upstream.

If startup stalls or restarts, check:

```bash
docker compose logs app
```

## First Browser Walkthrough

Start with these routes:

- `/`
- `/en-GB/shop`
- `/en-GB/shop/collections`
- `/en-GB/shop/products/harbor-cap`
- `/cart`
- `/checkout`
- `/account`
- `/admin`

Use these dev shortcuts for authenticated flows:

- `http://uk.localhost:8080/__dev/login/customer?next=/account`
- `http://uk.localhost:8080/__dev/login/admin?next=/admin`

The intended first-run path is:

1. open `/` and confirm the storefront home renders with CSS
2. open `http://uk.localhost:8080/en-GB/shop` and browse the UK catalog
3. open `http://fr.localhost:8080/fr-FR/events` and confirm the FR events-led edit
4. open `http://pl.localhost:8080/pl-PL/shop/products/harbor-scarf` and confirm the PL-only product path
5. add an item to cart from the UK or PL site
6. open `/cart`
7. open `/checkout`
8. use the dev login shortcut and inspect `/account`
9. use the admin shortcut and inspect `/admin`

That is a complete local walkthrough. You do not need a seeded SQL user to exercise account or
admin routes in the default development stack.

If you want to inspect the linked backend behaviour directly from the customer workspace, use:

```bash
./scripts/prepare-local-dev.sh
cargo run -p shoppr -- linked-backend loyalty-preview
cargo run -p shoppr -- linked-backend order-review
cargo run -p shoppr -- linked-backend crm-contact
```

## Local Stripe Testing

The default `.env.example` values keep Shoppr bootable without real Stripe credentials.

That default path is not a crippled mock. Shoppr falls back to the built-in local hosted
checkout stub so you can still exercise the checkout, confirmation, and account-order loop without
live Stripe credentials.

That is good enough for local UI development, account flows, catalog work, CMS work, and most
template/theme changes.

If you want real Stripe test-mode webhook behaviour:

1. set real test values in `.env`
2. start the stack
3. run Stripe CLI and forward events:

```bash
stripe listen --forward-to http://uk.localhost:8080/webhooks/commerce/payment-provider
```

Then update `.env` with the webhook secret that Stripe CLI gives you and restart the stack.

Use real Stripe test credentials only when you specifically want to validate third-party provider
handoff behaviour. They are not required for the default end-to-end local customer journey.

## Working On The Storefront

Most customer-facing work in Shoppr lives in:

- `templates/`
- `theme/frontend/`
- `theme/assets/`
- `content/page-types/`
- `auth/shoppr-auth/`

Typical edits:

- change storefront layout in `templates/layouts/`
- change page templates in `templates/pages/` or module-specific folders
- change CSS sources in `theme/frontend/`
- change Stimulus/Turbo entrypoints in `theme/frontend/*.ts`
- let the build emit compiled assets into `theme/assets/`
- add logos or images in `theme/assets/`
- adjust customer auth bindings in `auth/shoppr-auth/capabilities.toml`

Build the frontend assets from the checked-in Shoppr sources:

```bash
cd apps/shoppr
npm install
npm run build
```

Use the watcher while working on templates, controllers, or CSS:

```bash
cd apps/shoppr
npm run watch
```

The current asset split is:

- `theme/frontend/site.ts` and `theme/frontend/site.css`
  Storefront shell enhancements and styling.
- `theme/frontend/admin.ts` and `theme/frontend/admin.css`
  Operator/admin-only filters, copy helpers, and admin styling.
- `theme/frontend/cms-editor.ts` and `theme/frontend/cms-editor.css`
  CMS editor behavior such as block inventory controls and admin content tools.

After backend, code, or template changes, rebuild and restart:

```bash
docker compose up --build
```

If you want to use the nested Shoppr workspace directly instead of Docker Compose:

```bash
cd apps/shoppr
./scripts/prepare-local-dev.sh
cargo coil dev
```

If Postgres and Redis are already running and you only want the managed host loop without Compose,
use:

```bash
cd apps/shoppr
./scripts/prepare-local-dev.sh
cargo coil dev --skip-infra
```

The linked customer backend currently surfaces in three honest places:

- `cargo run -p shoppr -- describe`
- `cargo run -p shoppr -- linked-backend demo`
- the `/admin` dashboard section that renders linked plugin metadata from the runtime plan

## How Published Assets Work

Shoppr does not serve theme files by raw filename in the intended path.

During bootstrap, Coil publishes `theme/assets/*` through the asset pipeline and resolves them through the generated asset manifest. That is why template references use the asset helper instead of hard-coding deployment filenames.

For third-party developers, the important rule is simple:

- author source code in `theme/frontend/`
- run the Shoppr frontend build to emit compiled files into `theme/assets/`
- keep images, logos, and other static binaries in `theme/assets/`
- reference compiled files from templates through the template asset helper
- let Coil publish and fingerprint the final `theme/assets/*` outputs

The current Shoppr architecture is deliberately SSR-first:

- templates and fragments own the HTML
- Turbo enhances navigation and HTML-over-the-wire updates
- Stimulus attaches local controller behavior to rendered markup
- PostCSS and esbuild compile the final asset graph
- admin and CMS pages load their own bundles instead of forcing one global script onto every surface

## Adding A Customer Extension

WASM is not the default path for Shoppr-owned first-party logic anymore. Chapter 96 makes the
linked customer Rust crate the primary path when the code ships with the customer's own source
tree and build.

WASM remains the right path for bounded runtime-installed or third-party work, as described in:

- `docs/design/62-extension-packaging-versioning-and-distribution.md`
- `docs/design/80-customer-extensions-and-integration-patterns.md`

Use a WASM extension when the behaviour is bounded, replaceable, and should stay capability-scoped
at runtime rather than being linked into the customer build.

Examples:

- a custom CMS render hook
- a customer-specific CRM webhook consumer
- a branded admin widget
- a reporting export
- a background reconciliation job

Shoppr now includes a concrete bounded example under:

- `extensions/shoppr-waitlist-tools/`

That example is installed in the checked-in Shoppr app through the normal runtime extension path.
It exists to demonstrate chapter 80 coexistence honestly: Shoppr still keeps its first-party
checkout and webhook rules in linked Rust, while also running one bounded runtime-installed WASM
extension from the app manifest.

It shows:

- a real installed extension entry in `app.toml`
- a real package descriptor in `extensions/shoppr-waitlist-tools/package.toml`
- a checked-in WAT source that Shoppr compiles into the runtime-loaded `.wasm` artifact
- a bounded public render hook that executes on the checked-in CMS home page
- explicit separation from the linked Rust backend path in chapter 96

Shoppr bootstrap now compiles the checked-in `shoppr-waitlist-tools.wat` source into the pinned
artifact path before the runtime plan is built, so the example is no longer package-shape only.
The coexistence model is concrete:

1. linked Rust owns Shoppr's first-party checkout and verified-webhook logic
2. the app manifest also installs a bounded runtime extension from `extensions/`
3. the runtime serves both paths in the same checked-in app

If the customization starts owning shared data, deep transaction logic, or broadly reused product behaviour, it is usually the wrong thing to keep in WASM.

## Adding Custom Business Rules In Rust

This is the primary Shoppr customization path for customer-owned first-party logic, per
chapter 96.

Use a linked Rust crate when the behaviour ships with the customer app, needs direct hook
registration, or wants to participate in the customer build as first-party code. The design intent
for that is in:

- `docs/design/03-product-shape-core-official-modules-customer-apps.md`
- `docs/design/13-workspace-and-crate-layout.md`
- `docs/design/96-customer-root-workspaces-and-linked-rust-backends.md`

The practical rule is:

- use Shoppr templates, config, auth bindings, and linked customer Rust crates for first-party customer product logic
- use WASM extensions for bounded third-party or runtime-installed customization
- use native Rust modules when you need deeper access to transactions, shared data ownership, or widely reused domain logic
- use `crates/` only when the behaviour is becoming a reusable native module or needs platform-level ownership

For customer-specific native Rust work, the right place is a customer-app-owned crate in the
customer workspace, not random edits scattered through core.

Examples of native-Rust changes that are reasonable:

- a customer-owned integration adapter with strict transaction requirements
- a reusable module-level capability or workflow that is broader than one page render hook
- deeper commerce or membership business rules that need native domain access

Shoppr now includes a concrete checked-in example under:

- `crates/shoppr-backend/`

Read this as the first-party linked backend example.

This example is intentionally small but real:

- `src/lib.rs` is the primary chapter 96 example
- it exposes `shoppr_backend::plugin()` plus Shoppr-specific hook logic
- it shows how customer-owned rules stay in a linked Rust crate instead of starting in WASM or a separate service
- the checked-in Shoppr binary composes and registers that plugin directly during startup

Read these files first if you want to add customer-owned Rust logic:

- `crates/shoppr-backend/src/lib.rs`
- `backend/shoppr-loyalty-backend/src/lib.rs`
- `crates/shoppr-app/src/lib.rs`
- `crates/shoppr-bin/src/main.rs`

The example is meant to be copied and reshaped by third-party developers building a customer-owned
store on Coil without editing platform core.

If you want the shortest “how do I add my own Rust rule?” path:

1. add a new function or service in `backend/shoppr-loyalty-backend/src/lib.rs`
2. expose or wrap it through `crates/shoppr-backend/src/lib.rs`
3. expose the result through an existing template/render hook, checkout hook, or verified-webhook hook
4. add a focused unit test in the backend crate and, if needed, an integration assertion in `crates/shoppr-app/tests/`

The checked-in `review_order(...)` example is still the fastest rule to copy:

- define request/response types and a pure rule in `backend/shoppr-loyalty-backend/src/lib.rs`
- re-export or register it in `crates/shoppr-backend/src/lib.rs`
- keep the behaviour behind the linked plugin boundary
- add tests for both the pure rule and the Shoppr runtime surface that consumes it

If you want to work on the shared backend library directly without Docker Compose:

```bash
cd apps/shoppr
cargo run -p shoppr-loyalty-backend
cargo test -p shoppr-loyalty-backend
```

## Troubleshooting

If the stack is already running but you changed code:

```bash
docker compose up --build
```

If local state is stale:

```bash
docker compose down -v
docker compose up --build
```

If ports are already in use, the defaults are:

- app: `8080`
- Postgres: `15432`
- Redis: `16379`
- MinIO API: `9000`
- MinIO console: `9001`

If the app container restarts, read:

```bash
docker compose logs app
```

If CSS is missing, the first thing to check is whether asset publication succeeded during bootstrap.

## Where To Go Next

If you want to understand the design behind Shoppr, read:

- `docs/design/77-customer-themes-templates-and-frontends.md`
- `docs/design/78-customer-specific-configuration-and-content-models.md`
- `docs/design/79-customer-specific-auth-and-capability-mappings.md`
- `docs/design/80-customer-extensions-and-integration-patterns.md`
- `docs/design/93-example-customer-app.md`

If you want to understand the platform, do not start in core. Start by getting Shoppr running, click through the app, then trace outward into the modules and runtime.
