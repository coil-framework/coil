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

The first boot will take longer because the image has to compile the workspace.

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

The compose stack also exposes local backing services:

- PostgreSQL: `postgres://davenda:devpass@localhost:5432/davenda_harbor_shop`
- Redis: `redis://localhost:6379`
- MinIO API: `http://localhost:9000`
- MinIO Console: `http://localhost:9001`
  - username: `minio`
  - password: `minio123`

## Local Dev Contract

This stack uses [platform.dev.toml](/Users/zcourts/projects/worka/davenda/apps/harbor-shop/platform.dev.toml), which is intentionally local-friendly:

- `environment = "development"` so HTTP object-store endpoints are allowed
- cookie `secure` flags are disabled so the site works on plain `http://localhost:8080`
- TLS is marked `external` so local startup does not pretend to run ACME
- `assets.cdn_base_url` points at the local MinIO bucket

The production-shaped sample config remains in [platform.toml](/Users/zcourts/projects/worka/davenda/apps/harbor-shop/platform.toml). The compose stack does not modify it.

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

## Current Limits

This stack is meant to get a developer into the real checked-in app quickly, not to hide product gaps.

- Checkout is still provider-aware but not a full live Stripe handoff.
- There is no turnkey seeded admin login in this compose flow.
- Account and admin routes exist, but authenticated flows still need explicit session/bootstrap work outside this quick-start stack.

That said, the public storefront loop is real enough to inspect templates, route coverage, asset delivery, cart state, checkout state, event pages, and the local infrastructure contract in one run.
