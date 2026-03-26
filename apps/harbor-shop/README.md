# Harbor Shop

Harbor Shop is the reference Davenda customer app in this repository.

It is not the platform itself. It is an example deployable product built on top of Davenda:

- it selects official modules
- it provides the customer app manifest and platform config
- it owns the storefront templates and theme assets
- it carries customer-specific auth bindings
- it is the fastest way to see what a real Davenda app looks like

If you are new to the repo, start here.

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

`docker/`, `Dockerfile`, `docker-compose.yml`
- the local developer stack

## What Harbor Shop Demonstrates

Harbor Shop is meant to show the boundary described in `docs/design`:

- Davenda core provides the runtime, routing, storage, cache, auth execution, jobs, TLS, and asset publication
- official modules provide reusable product batteries like CMS, commerce, memberships, events, admin, and ops
- Harbor Shop provides composition, branding, templates, app policy, and customer-specific behavior

That is why this folder matters. It is the customer-app layer, not a random demo directory.

## Prerequisites

You need:

- Docker Desktop or Docker Engine plus the Compose plugin
- a working `docker compose` command
- optionally, Stripe CLI if you want to test real local webhook forwarding

You do not need to install Postgres, Redis, or MinIO manually for the default local run.

## Quick Start

From the repo root:

```bash
cd apps/harbor-shop
cp .env.example .env
docker compose up --build
```

Then open:

- `http://localhost:8080/`
- `http://localhost:8080/__dev`

The `__dev` page gives you one-click local login shortcuts for the checked-in customer and admin paths.

## What To Expect During Startup

These MinIO lines are expected:

```text
Added `local` successfully.
Bucket created successfully `local/harbor-shop`.
Access permission for `local/harbor-shop` is set to `download`
```

That is the bootstrap job creating the local object-store bucket and making published theme assets downloadable in the dev stack.

The `app` container then does four things:

1. validates `platform.dev.toml`
2. applies migrations
3. publishes theme assets
4. starts `platform dev server`

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

## Local Stripe Testing

The default `.env.example` values keep Harbor Shop bootable without real Stripe credentials.

That is good enough for local UI development, account flows, catalog work, CMS work, and most template/theme changes.

If you want real Stripe test-mode webhook behavior:

1. set real test values in `.env`
2. start the stack
3. run Stripe CLI and forward events:

```bash
stripe listen --forward-to http://localhost:8080/webhooks/commerce/payment-provider
```

Then update `.env` with the webhook secret that Stripe CLI gives you and restart the stack.

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

## How Published Assets Work

Harbor Shop does not serve theme files by raw filename in the intended path.

During bootstrap, Davenda publishes `theme/assets/*` through the asset pipeline and resolves them through the generated asset manifest. That is why template references use the asset helper instead of hard-coding deployment filenames.

For third-party developers, the important rule is simple:

- put stable source assets in `theme/assets/`
- reference them from templates through the template asset helper
- let Davenda publish and fingerprint them

## Adding A Customer Extension

The default customer-specific customization path is a WASM extension, as described in:

- `docs/design/62-extension-packaging-versioning-and-distribution.md`
- `docs/design/80-customer-extensions-and-integration-patterns.md`

Use an extension when the behavior is specific to one app and does not deserve promotion into a shared native module.

Examples:

- a custom CMS render hook
- a customer-specific CRM webhook consumer
- a branded admin widget
- a reporting export
- a background reconciliation job

The Harbor Shop app does not currently ship a ready-made extension package generator inside this folder. The workflow today is:

1. build a WASM extension package against Davenda’s extension contracts
2. place the compiled artifact under `apps/harbor-shop/extensions/`
3. keep the extension scoped to Harbor Shop rather than turning it into an undocumented platform dependency

If the customization starts owning shared data, deep transaction logic, or broadly reused product behavior, it is usually the wrong thing to keep in WASM.

## Adding Custom Business Rules In Rust

Not every customization belongs in a WASM extension.

Use native Rust code when the behavior needs deeper runtime integration, broader reuse, or stronger operational guarantees. The design intent for that is in:

- `docs/design/03-product-shape-core-official-modules-customer-apps.md`
- `docs/design/13-workspace-and-crate-layout.md`

The practical rule is:

- use Harbor Shop templates, config, auth bindings, and WASM extensions for app-specific presentation and bounded behavior
- use native Rust modules or adapters when you need deeper access to transactions, render pipeline internals, shared data ownership, or widely reused domain logic

For customer-specific native Rust work, the right place is a customer-app-owned package or adapter crate in the workspace, not random edits scattered through core.

Examples of native-Rust changes that are reasonable:

- a customer-owned integration adapter with strict transaction requirements
- a reusable module-level capability or workflow that is broader than one page render hook
- deeper commerce or membership business rules that need native domain access

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
