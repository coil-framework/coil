# Harbor Shop

Harbor Shop is the reference Davenda customer app in this repository.

It is not the platform itself. It is an example deployable product built on top of Davenda:

- it selects official modules
- it provides the customer app manifest and platform config
- it owns the storefront templates and theme assets
- it carries customer-specific auth bindings
- it is the fastest way to see what a real Davenda app looks like

If you are new to the repo, start here.

For local development, Harbor Shop is intentionally turnkey for the full public and authenticated
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

`auth/harbor-auth/`
- the customer app auth package
- this is where app-specific capability mappings live

`content/`
- customer-owned content model definitions such as page types

`templates/`
- the HTML-first template tree for storefront, account, CMS, and admin pages

`theme/`
- the Harbor Shop theme assets
- CSS, images, and other published frontend assets live here

`extensions/`
- the deployment target for customer-specific WASM extension artifacts

`backend/`
- customer-app-owned Rust backend example code
- the primary example is a linked customer crate consistent with chapter 96
- the optional HTTP sidecar adapter remains here only for cases that truly need a process boundary

`Cargo.toml`, `Cargo.lock`, `crates/`
- the Harbor Shop nested Cargo workspace
- `crates/harbor-shop-bin` is the customer binary composition root
- `crates/harbor-shop-app` owns manifest/config/auth loading plus Harbor Shop runtime composition
- `crates/harbor-shop-backend` is the linked customer Rust backend registered into the customer binary

`docker/`, `Dockerfile`, `docker-compose.yml`
- the local developer stack

## What Harbor Shop Demonstrates

Harbor Shop is meant to show the boundary described in `docs/design`:

- Davenda core provides the runtime, routing, storage, cache, auth execution, jobs, TLS, and asset publication
- official modules provide reusable product batteries like CMS, commerce, memberships, events, admin, and ops
- Harbor Shop provides composition, branding, templates, app policy, and customer-specific behavior

That is why this folder matters. It is the customer-app layer, not a random demo directory.

## Harbor Shop As A Customer-Root Workspace

Harbor Shop now has its own nested Cargo workspace in this folder.

That is the chapter 96 shape in repo form:

- Davenda is modeled here as normal upstream `0.1.0` dependencies
- Harbor Shop owns a binary crate that links the official modules it needs
- Harbor Shop owns a linked backend crate that registers customer-specific behavior through public Davenda APIs
- the optional sidecar adapter still exists, but it is no longer the primary Rust integration story

The committed workspace is intentionally free of `patch.crates-io` overlays and other repo-local
dependency rewrites. Harbor Shop is checked in as a normal customer project that can resolve
Davenda from an upstream registry or pinned git source.

The Docker story now follows the same split:

- `docker compose up --build` uses `apps/harbor-shop` as the Docker build context and expects
  Davenda crates plus the Harbor customer binary to resolve like normal upstream dependencies
- `docker compose -f docker-compose.yml -f docker-compose.repo.yml up --build` is the explicit
  repo-maintainer override when you are building Harbor Shop against this monorepo before those
  upstream packages are published

If you are iterating on Harbor Shop from inside the Davenda repository before those upstream crates
are published, keep that override local and uncommitted. The supported maintainer path is:

```bash
./scripts/prepare-local-dev.sh
```

That writes `apps/harbor-shop/.cargo/config.toml` with repo-local path patches. The file is
ignored by git and exists only so this checked-in example can build against the current Davenda
workspace without polluting the committed customer manifest.

From `apps/harbor-shop`:

```bash
./scripts/prepare-local-dev.sh
cargo run -p harbor-shop -- describe
cargo run -p harbor-shop -- validate
cargo run -p harbor-shop -- migrate apply --dry-run
cargo run -p harbor-shop -- assets publish
```

That prints the active app root, config, linked modules, and the linked customer plugin ids added
by the Harbor Shop workspace.

The checked-in linked plugin in this workspace is:

- `Harbor Shop Linked Backend` (`harbor-shop-backend`)

Once Harbor Shop is running, the same linked-backend shape is visible inside the app itself:

- `/admin` now renders the linked customer plugin metadata from the runtime plan itself
- `cargo run -p harbor-shop -- describe` prints the linked plugin ids in the customer workspace
- `cargo run -p harbor-shop -- linked-backend demo` executes the checked-in linked backend rules directly from the customer workspace without the optional sidecar

From `apps/harbor-shop`, the shortest concrete linked-backend walkthrough is:

```bash
./scripts/prepare-local-dev.sh
cargo run -p harbor-shop -- linked-backend describe
cargo run -p harbor-shop -- linked-backend demo
```

That path stays entirely inside the customer-root workspace. It exercises the exact linked Rust
crate Harbor Shop compiles into the app and prints real loyalty-preview, checkout-review, and
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
docker compose up --build
```

That is the honest customer-project path. It builds Harbor Shop from this folder only.

If you are a Davenda maintainer building Harbor Shop from inside this repository before upstream
crate publication, use the explicit repo override:

```bash
cp .env.example .env
docker compose -f docker-compose.yml -f docker-compose.repo.yml up --build
```

If you also want the optional sidecar adapter for the checked-in Rust backend example running locally:

```bash
docker compose --profile backend-example up --build
```

Then open:

- `http://localhost:8080/`
- `http://localhost:8080/__dev`
- `http://localhost:8081/`

The `__dev` page gives you one-click local login shortcuts for the checked-in customer and admin
paths. This is the local bootstrap mechanism for authenticated walkthroughs; Harbor Shop does not
depend on a seeded admin password for first-run development.

## What To Expect During Startup

These MinIO lines are expected:

```text
Added `local` successfully.
Bucket created successfully `local/harbor-shop`.
Access permission for `local/harbor-shop` is set to `download`
```

That is the bootstrap job creating the local object-store bucket and making published theme assets downloadable in the dev stack.

The `app` container now runs Harbor Shop's own lifecycle command:

1. Harbor builds and validates the customer runtime from `platform.dev.toml`
2. Harbor applies pending executable migrations through the customer binary
3. Harbor publishes theme assets through the same customer runtime build path
4. Harbor starts the storefront/admin server

You can run the same end-to-end lifecycle directly from the customer workspace:

```bash
./scripts/prepare-local-dev.sh
cargo run -p harbor-shop -- up
```

The committed Cargo workspace stays upstream-clean. The default checked-in `docker compose up`
path now uses only the Harbor Shop folder as its Docker build context. The separate
`docker-compose.repo.yml` override is the only in-repo maintainer convenience path, and it is
explicit about using the Davenda monorepo plus local Cargo patching before the `davenda-*` crates
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

- `http://localhost:8080/__dev/login/customer?next=/account`
- `http://localhost:8080/__dev/login/admin?next=/admin`

The intended first-run path is:

1. open `/` and confirm the storefront home renders with CSS
2. open `/en-GB/shop` and browse the catalog
3. open a product detail page
4. add an item to cart
5. open `/cart`
6. open `/checkout`
7. use the dev login shortcut and inspect `/account`
8. use the admin shortcut and inspect `/admin`

That is a complete local walkthrough. You do not need a seeded SQL user to exercise account or
admin routes in the default development stack.

If you started the optional backend-example profile, also visit:

- `http://localhost:8081/`
- `http://localhost:8081/health`
- `POST http://localhost:8081/api/loyalty/preview`
- `POST http://localhost:8081/api/orders/review`

If you want to inspect the same linked backend behavior without running the optional sidecar, use:

```bash
./scripts/prepare-local-dev.sh
cargo run -p harbor-shop -- linked-backend loyalty-preview
cargo run -p harbor-shop -- linked-backend order-review
cargo run -p harbor-shop -- linked-backend crm-contact
```

## Local Stripe Testing

The default `.env.example` values keep Harbor Shop bootable without real Stripe credentials.

That default path is not a crippled mock. Harbor Shop falls back to the built-in local hosted
checkout stub so you can still exercise the checkout, confirmation, and account-order loop without
live Stripe credentials.

That is good enough for local UI development, account flows, catalog work, CMS work, and most
template/theme changes.

If you want real Stripe test-mode webhook behavior:

1. set real test values in `.env`
2. start the stack
3. run Stripe CLI and forward events:

```bash
stripe listen --forward-to http://localhost:8080/webhooks/commerce/payment-provider
```

Then update `.env` with the webhook secret that Stripe CLI gives you and restart the stack.

Use real Stripe test credentials only when you specifically want to validate third-party provider
handoff behavior. They are not required for the default end-to-end local customer journey.

## Working On The Storefront

Most customer-facing work in Harbor Shop lives in:

- `templates/`
- `theme/assets/`
- `content/page-types/`
- `auth/harbor-auth/`

Typical edits:

- change storefront layout in `templates/layouts/`
- change page templates in `templates/pages/` or module-specific folders
- change CSS in `theme/assets/site.css`
- add logos or images in `theme/assets/`
- adjust customer auth bindings in `auth/harbor-auth/capabilities.toml`

After code or template changes, rebuild and restart:

```bash
docker compose up --build
```

If you want to use the nested Harbor Shop workspace directly instead of Docker Compose:

```bash
cd apps/harbor-shop
./scripts/prepare-local-dev.sh
cargo run -p harbor-shop -- describe
DATABASE_URL=postgres://davenda:devpass@127.0.0.1:5438/davenda_harbor_shop \
REDIS_URL=redis://127.0.0.1:6379 \
OBJECT_STORE_URL='endpoint_url="http://127.0.0.1:9000"
bucket="harbor-shop"
region="us-east-1"
access_key_id="minio"
secret_access_key="minio123"' \
DAVENDA_COOKIE_SECRET=01234567012345670123456701234567 \
DAVENDA_CSRF_SECRET=76543210765432107654321076543210 \
cargo run -p harbor-shop -- up --config platform.dev.toml
```

The linked customer backend currently surfaces in two honest places:

- `cargo run -p harbor-shop -- describe`
- `cargo run -p harbor-shop -- linked-backend demo`
- the `/admin` dashboard section that renders linked plugin metadata from the runtime plan

## How Published Assets Work

Harbor Shop does not serve theme files by raw filename in the intended path.

During bootstrap, Davenda publishes `theme/assets/*` through the asset pipeline and resolves them through the generated asset manifest. That is why template references use the asset helper instead of hard-coding deployment filenames.

For third-party developers, the important rule is simple:

- put stable source assets in `theme/assets/`
- reference them from templates through the template asset helper
- let Davenda publish and fingerprint them

## Adding A Customer Extension

WASM is not the default path for Harbor Shop-owned first-party logic anymore. Chapter 96 makes the
linked customer Rust crate the primary path when the code ships with the customer's own source
tree and build.

WASM remains the right path for bounded runtime-installed or third-party work, as described in:

- `docs/design/62-extension-packaging-versioning-and-distribution.md`
- `docs/design/80-customer-extensions-and-integration-patterns.md`

Use a WASM extension when the behavior is bounded, replaceable, and should stay capability-scoped
at runtime rather than being linked into the customer build.

Examples:

- a custom CMS render hook
- a customer-specific CRM webhook consumer
- a branded admin widget
- a reporting export
- a background reconciliation job

Harbor Shop now includes a concrete bounded example under:

- `extensions/harbor-waitlist-tools/`

That example is installed in the checked-in Harbor app through the normal runtime extension path.
It exists to demonstrate chapter 80 coexistence honestly: Harbor Shop still keeps its first-party
checkout and webhook rules in linked Rust, while also running one bounded runtime-installed WASM
extension from the app manifest.

It shows:

- a real installed extension entry in `app.toml`
- a real package descriptor in `extensions/harbor-waitlist-tools/package.toml`
- a checked-in WAT source that Harbor compiles into the runtime-loaded `.wasm` artifact
- a bounded public render hook that executes on the checked-in CMS home page
- explicit separation from the linked Rust backend path in chapter 96

Harbor bootstrap now compiles the checked-in `harbor-waitlist-tools.wat` source into the pinned
artifact path before the runtime plan is built, so the example is no longer package-shape only.
The coexistence model is concrete:

1. linked Rust owns Harbor Shop's first-party checkout and verified-webhook logic
2. the app manifest also installs a bounded runtime extension from `extensions/`
3. the runtime serves both paths in the same checked-in app

If the customization starts owning shared data, deep transaction logic, or broadly reused product behavior, it is usually the wrong thing to keep in WASM.

## Adding Custom Business Rules In Rust

This is the primary Harbor Shop customization path for customer-owned first-party logic, per
chapter 96.

Use a linked Rust crate when the behavior ships with the customer app, needs direct hook
registration, or wants to participate in the customer build as first-party code. The design intent
for that is in:

- `docs/design/03-product-shape-core-official-modules-customer-apps.md`
- `docs/design/13-workspace-and-crate-layout.md`
- `docs/design/96-customer-root-workspaces-and-linked-rust-backends.md`

The practical rule is:

- use Harbor Shop templates, config, auth bindings, and linked customer Rust crates for first-party customer product logic
- use WASM extensions for bounded third-party or runtime-installed customization
- use native Rust modules when you need deeper access to transactions, shared data ownership, or widely reused domain logic
- use the optional sidecar adapter under `apps/harbor-shop/backend/` only when the linked crate genuinely needs a separate HTTP/process boundary
- use `crates/` only when the behavior is becoming a reusable native module or needs platform-level ownership

For customer-specific native Rust work, the right place is a customer-app-owned crate in the
customer workspace, not random edits scattered through core.

Examples of native-Rust changes that are reasonable:

- a customer-owned integration adapter with strict transaction requirements
- a reusable module-level capability or workflow that is broader than one page render hook
- deeper commerce or membership business rules that need native domain access

Harbor Shop now includes a concrete checked-in example under:

- `backend/harbor-loyalty-backend/`

Read this as a linked customer-backend example first and an optional sidecar second.

This example is intentionally small but real:

- `src/lib.rs` is the primary chapter 96 example
- it exposes `harbor_loyalty_backend::plugin()` plus Harbor Shop-specific hook logic
- it shows how customer-owned rules stay in a linked Rust crate instead of starting in WASM or a sidecar
- `src/http.rs` and `src/main.rs` are the optional sidecar adapter around the same linked crate
- the sidecar path remains useful for external webhook/process integration, but it is not the primary model

Read these files first if you want to add customer-owned Rust logic:

- `backend/README.md`
- `backend/harbor-loyalty-backend/README.md`
- `backend/harbor-loyalty-backend/src/lib.rs`
- `backend/harbor-loyalty-backend/src/http.rs`

To run the optional sidecar adapter with the local stack:

```bash
docker compose --profile backend-example up --build
```

Then exercise it with the checked-in sample payloads:

```bash
curl -sS \
  -X POST http://localhost:8081/api/loyalty/preview \
  -H 'content-type: application/json' \
  --data @backend/harbor-loyalty-backend/requests/loyalty-preview.json
```

```bash
curl -sS \
  -X POST http://localhost:8081/api/orders/review \
  -H 'content-type: application/json' \
  --data @backend/harbor-loyalty-backend/requests/order-review.json
```

```bash
curl -sS \
  -X POST http://localhost:8081/webhooks/crm/contact-updated \
  -H 'content-type: application/json' \
  -H "x-harbor-backend-secret: ${HARBOR_BACKEND_WEBHOOK_SECRET:-harbor-backend-dev-secret}" \
  --data @backend/harbor-loyalty-backend/requests/contact-updated.json
```

The example is meant to be copied and reshaped by third-party developers who need customer-owned
Rust logic linked into Harbor Shop without leaking into platform core. The optional sidecar is only
the transport wrapper for cases that need it.

If you want the shortest “how do I add my own Rust rule?” path, start with the linked crate API in
`src/lib.rs`, then optionally add an HTTP adapter if the integration truly needs one.

The checked-in `review_order(...)` example is still the fastest rule to copy:

- define request/response types and a pure rule in `src/lib.rs`
- add the thin HTTP adapter in `src/http.rs`
- add a sample payload under `requests/`
- add tests for both the pure rule and the route

To work on the example crate without Docker Compose:

```bash
cd apps/harbor-shop
cargo run -p harbor-loyalty-backend
cargo test -p harbor-loyalty-backend
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
- Postgres: `5438`
- Redis: `6379`
- MinIO API: `9000`
- MinIO console: `9001`

If the app container restarts, read:

```bash
docker compose logs app
```

If CSS is missing, the first thing to check is whether asset publication succeeded during bootstrap.

## Where To Go Next

If you want to understand the design behind Harbor Shop, read:

- `docs/design/77-customer-themes-templates-and-frontends.md`
- `docs/design/78-customer-specific-configuration-and-content-models.md`
- `docs/design/79-customer-specific-auth-and-capability-mappings.md`
- `docs/design/80-customer-extensions-and-integration-patterns.md`
- `docs/design/93-example-customer-app.md`

If you want to understand the platform, do not start in core. Start by getting Harbor Shop running, click through the app, then trace outward into the modules and runtime.
