# Shoppr Customer Backend Example

This folder contains customer-app-owned native Rust backend examples for Shoppr.

The primary path here is the chapter 96 model: a linked customer Rust crate that implements
customer hooks and ships as first-party code in the customer build.

The optional Axum sidecar remains in the example only as a bounded secondary path for cases that
truly benefit from a separate HTTP/process boundary.

The checked-in example is:

- `shoppr-loyalty-backend/`
  - a linked Rust crate exposing Shoppr-specific backend rules through `plugin()`
  - includes an optional HTTP adapter for sidecar-style deployment
  - includes a crate-level tutorial in `shoppr-loyalty-backend/README.md`

## Choose The Right Customization Path

Use:

- `templates/`, `theme/`, `content/`, and app config for presentation and app policy
- linked customer Rust crates for first-party Shoppr-specific backend logic
- `extensions/` for bounded WASM-based third-party or runtime-installed customization
- the optional HTTP adapter under `backend/<service>/src/http.rs` only when a separate process boundary is actually desirable
- `crates/` only when the behavior is becoming a reusable native module or needs deeper platform ownership

The important rule is to keep customer-specific code contained. Do not scatter Shoppr-specific
logic through core/runtime code just because it is Rust.

## Linked Crate First

The file to read first is:

- `shoppr-loyalty-backend/src/lib.rs`

That library is the primary example. It keeps Shoppr-specific logic in normal Rust functions
and exposes a `plugin()` entrypoint that matches the direction in
`docs/design/96-customer-root-workspaces-and-linked-rust-backends.md`.

High-level intended shape:

```rust
davenda_all::builder()
    .with_customer_plugin(shoppr_loyalty_backend::plugin())
    .run_from_env()
```

Shoppr now exposes that linked plugin shape in two concrete places:

- the customer workspace registers the plugin into its runtime composition
- the customer workspace can run the linked backend sample requests directly with `cargo run -p shoppr -- linked-backend demo`
- the Shoppr admin dashboard renders the linked plugin metadata from the runtime plan itself
- `cargo run -p shoppr -- describe` prints the linked plugin ids from the customer workspace

Before you touch the optional sidecar adapter, use the customer-root demo path:

```bash
cd apps/shoppr
./scripts/prepare-local-dev.sh
cargo run -p shoppr -- linked-backend describe
cargo run -p shoppr -- linked-backend demo
```

That exercises the same linked Rust crate Shoppr compiles into its customer binary. It makes
the chapter 96 path concrete without forcing a separate process.

## Optional Sidecar Adapter

From `apps/shoppr`:

```bash
docker compose --profile backend-example up --build
```

That uses Shoppr itself as the Docker build context and starts the normal Shoppr stack
plus the optional sidecar adapter on
`http://localhost:8081`.

If you are building Shoppr against the live Davenda monorepo before upstream publication, use
the explicit repo override instead:

```bash
docker compose -f docker-compose.yml -f docker-compose.repo.yml --profile backend-example up --build
```

Useful routes:

- `GET http://localhost:8081/`
- `GET http://localhost:8081/health`
- `POST http://localhost:8081/api/loyalty/preview`
- `POST http://localhost:8081/api/orders/review`
- `POST http://localhost:8081/webhooks/crm/contact-updated`

You can also run just the optional sidecar adapter without Docker Compose:

```bash
cd apps/shoppr
cargo run -p shoppr-loyalty-backend
```

## Local Curl Examples

Preview a customer-specific loyalty decision:

```bash
curl -sS \
  -X POST http://localhost:8081/api/loyalty/preview \
  -H 'content-type: application/json' \
  --data @backend/shoppr-loyalty-backend/requests/loyalty-preview.json
```

Preview a Shoppr-specific fulfilment decision:

```bash
curl -sS \
  -X POST http://localhost:8081/api/orders/review \
  -H 'content-type: application/json' \
  --data @backend/shoppr-loyalty-backend/requests/order-review.json
```

Exercise the signed webhook path:

```bash
curl -sS \
  -X POST http://localhost:8081/webhooks/crm/contact-updated \
  -H 'content-type: application/json' \
  -H 'x-harbor-backend-secret: harbor-backend-dev-secret' \
  --data @backend/shoppr-loyalty-backend/requests/contact-updated.json
```

## What This Example Is Showing

This is not a replacement for Davenda runtime modules. It is an example of customer-owned Rust
logic that lives with the customer app instead of being scattered through core.

The example intentionally demonstrates three things:

- Shoppr-specific business rules written in plain Rust
- a linked customer-backend crate as the primary ownership boundary
- an optional sidecar adapter for the same rules when a process boundary is operationally useful

The `review_order(...)` rule in `src/lib.rs` is the clearest “copy this” path for developers who
need to add one more Shoppr-specific backend rule. The Axum route is secondary.

It also demonstrates the code split we want third-party developers to copy:

- `src/lib.rs` for the linked customer-backend/plugin logic
- `src/http.rs` for the optional sidecar route wiring and validation
- `src/main.rs` for optional process bootstrap

If you are starting from scratch, read `shoppr-loyalty-backend/README.md` first. It is the
step-by-step tutorial for how this example is structured and how to add your own route.

If you only need a bounded page hook, widget, or lightweight integration point, prefer a WASM
extension. If you need customer-owned first-party Rust logic, start with the linked crate here.
Reach for the sidecar adapter only when the transport boundary is part of the real problem.
