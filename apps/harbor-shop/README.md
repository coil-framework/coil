# Harbor Shop

Harbor Shop is the checked-in customer app that exercises the Davenda storefront, cart, checkout, events, memberships, and admin surfaces in one place. This folder now ships with a one-command local stack, so a developer can move from clone to running store without hand-wiring Postgres, Redis, or object storage.

## Quick Start

From the repository root:

```bash
cd apps/harbor-shop
docker compose up
```

What `docker compose up` does for you:

- builds a Harbor Shop application image with the current workspace code
- starts PostgreSQL on `localhost:5432`
- starts Redis on `localhost:6379`
- starts MinIO on `localhost:9000` with the console on `localhost:9001`
- creates the `harbor-shop` object-store bucket with anonymous download enabled for local asset delivery
- validates the Harbor Shop config
- applies migrations automatically
- publishes theme assets automatically
- starts the Davenda dev server on `http://localhost:8080`
- keeps localhost checkout usable with the built-in development payment stub until real Stripe test keys are supplied
- exposes development-only login shortcuts so you can inspect customer and admin flows immediately

The first boot will take longer because the image has to compile the workspace.
The default compose file boots with placeholder Stripe keys. That keeps the checked-in hosted checkout path active, but localhost testing still works because the runtime swaps in a built-in development checkout stub until you override those placeholders with real Stripe test credentials.

## What To Open

Once the app is up, these are the main URLs to verify:

- `http://localhost:8080/en-GB/pages/home`
- `http://localhost:8080/en-GB/shop`
- `http://localhost:8080/en-GB/shop/collections`
- `http://localhost:8080/en-GB/shop/collections/featured`
- `http://localhost:8080/en-GB/shop/products/harbor-cap`
- `http://localhost:8080/cart`
- `http://localhost:8080/checkout`
- `http://localhost:8080/checkout/confirmation`
- `http://localhost:8080/en-GB/events`
- `http://localhost:8080/__dev`
- `http://localhost:8080/__dev/login/customer?next=/account`
- `http://localhost:8080/__dev/login/admin?next=/admin`

The compose stack also exposes local backing services:

- PostgreSQL: `postgres://davenda:devpass@localhost:5432/davenda_harbor_shop`
- Redis: `redis://localhost:6379`
- MinIO API: `http://localhost:9000`
- MinIO Console: `http://localhost:9001`
  - username: `minio`
  - password: `minio123`

To exercise a real Stripe Checkout redirect instead of the built-in local stub, export your test keys before starting the stack:

```bash
export STRIPE_PUBLISHABLE_KEY=pk_test_your_key
export STRIPE_SECRET_KEY=sk_test_your_key
docker compose up --build
```

## Local Dev Contract

This stack uses [platform.dev.toml](/Users/zcourts/projects/worka/davenda/apps/harbor-shop/platform.dev.toml), which is intentionally local-friendly:

- `environment = "development"` so HTTP object-store endpoints are allowed
- cookie `secure` flags are disabled so the site works on plain `http://localhost:8080`
- TLS is marked `external` so local startup does not pretend to run ACME
- `assets.cdn_base_url` points at the local MinIO bucket
- `wasm.secret_bindings` exposes the Stripe secret key the hosted checkout handoff actually reads at runtime when you replace the placeholder key
- development-only `__dev` session routes let you enter the checked-in customer and admin journeys without manual auth bootstrap

The production-shaped sample config remains in [platform.toml](/Users/zcourts/projects/worka/davenda/apps/harbor-shop/platform.toml). The compose stack does not modify it.

## Local Personas

For a fast browser check of the authenticated surfaces, use these development-only shortcuts:

- `http://localhost:8080/__dev/login/customer?next=/account`
  - issues a local browser session for the sample customer principal and lands on the account area
- `http://localhost:8080/__dev/login/admin?next=/admin`
  - issues a local browser session for the sample admin principal and lands on the admin area

These routes exist only in the local development config. They are not present in the production sample config.

## Common Commands

Rebuild after Rust or template changes:

```bash
docker compose up --build
```

Stop the stack:

```bash
docker compose down
```

Stop the stack and wipe all local state:

```bash
docker compose down -v
```
