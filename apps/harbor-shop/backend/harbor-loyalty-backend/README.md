# Harbor Loyalty Backend

`harbor-loyalty-backend` is the checked-in Harbor Shop example for chapter 96: a linked
customer-owned Rust backend crate that holds first-party customer logic.

Use it when you need custom Rust code for one customer store and that work does not belong in:

- templates or config
- a bounded WASM extension under `apps/harbor-shop/extensions/` for third-party/runtime-installed work
- a reusable first-party/native module under `crates/`

This example stays intentionally small, but it demonstrates the full path a third-party developer
needs:

- where Harbor Shop-specific Rust code lives
- how a linked customer crate exposes hook-style logic through `plugin()`
- how the same crate can optionally be wrapped in an HTTP sidecar
- how signed inbound integration traffic is handled when that sidecar is warranted
- how to add tests for the pure logic and the optional HTTP surface

## What Lives Where

`src/lib.rs`
- the primary linked-crate example
- Harbor Shop business logic, request/response types, and `plugin()` live here
- this is where customer-specific rules should start

`src/http.rs`
- the optional HTTP adapter
- route registration, request validation, and signed webhook checks live here when a sidecar is justified

`src/main.rs`
- optional process bootstrap
- reads `HARBOR_BACKEND_*` environment variables and starts the Axum server

`requests/*.json`
- checked-in sample payloads for manual `curl` testing

`Dockerfile`
- builds the service for the `backend-example` Docker Compose profile

## How It Connects To Harbor Shop

The primary connection path is the linked crate:

1. customer-owned logic lives in `src/lib.rs`
2. `plugin()` is the intended registration point for a customer workspace/binary
3. the customer binary links this crate and registers the hooks at startup
4. Harbor Shop-specific behavior stays in customer-owned Rust rather than starting in WASM or a sidecar

The optional sidecar path is secondary:

1. `apps/harbor-shop/docker-compose.yml` defines the optional `harbor-backend-example` service
2. that service builds this crate and exposes it on `http://localhost:8081`
3. `src/main.rs` loads `HARBOR_BACKEND_*` settings
4. `src/http.rs` maps HTTP routes onto the same rules from `src/lib.rs`

That is the intended Harbor Shop pattern now: linked crate first, sidecar only when a process
boundary is genuinely useful.

## Linked Crate First

Read `src/lib.rs` first. It is the primary example.

High-level intended registration shape:

```rust
davenda_all::builder()
    .with_customer_plugin(harbor_loyalty_backend::plugin())
    .run_from_env()
```

The exact stable SDK/bootstrap layer is still platform work, but this example crate is already
structured around that ownership model. In the current Harbor Shop workspace, the linked plugin is
visible through the Harbor customer binary’s `describe` command, the
`cargo run -p harbor-shop -- linked-backend demo` customer-workspace walkthrough, and the admin
dashboard’s runtime-backed plugin metadata panel once the app is running.

From `apps/harbor-shop`, the linked-crate-first path is:

```bash
./scripts/prepare-local-dev.sh
cargo run -p harbor-shop -- linked-backend describe
cargo run -p harbor-shop -- linked-backend demo
```

That path does not need the optional sidecar. It loads the checked-in sample requests and executes
the linked backend directly through the Harbor customer workspace.

## Optional Sidecar Adapter

From `apps/harbor-shop`:

```bash
cargo run -p harbor-loyalty-backend
```

By default the optional sidecar binds to `0.0.0.0:8081`.

To run it with the full Harbor Shop stack:

```bash
cd apps/harbor-shop
docker compose --profile backend-example up --build
```

If you are building Harbor Shop against the live Davenda monorepo before upstream publication, use
the explicit repo override:

```bash
cd apps/harbor-shop
docker compose -f docker-compose.yml -f docker-compose.repo.yml --profile backend-example up --build
```

Useful routes:

- `GET http://localhost:8081/`
- `GET http://localhost:8081/health`
- `POST http://localhost:8081/api/loyalty/preview`
- `POST http://localhost:8081/api/orders/review`
- `POST http://localhost:8081/webhooks/crm/contact-updated`

## Exercise The Example

Preview a Harbor Shop-specific loyalty rule:

```bash
curl -sS \
  -X POST http://localhost:8081/api/loyalty/preview \
  -H 'content-type: application/json' \
  --data @apps/harbor-shop/backend/harbor-loyalty-backend/requests/loyalty-preview.json
```

Exercise the fail-closed webhook route:

```bash
curl -sS \
  -X POST http://localhost:8081/webhooks/crm/contact-updated \
  -H 'content-type: application/json' \
  -H "x-harbor-backend-secret: ${HARBOR_BACKEND_WEBHOOK_SECRET:-harbor-backend-dev-secret}" \
  --data @apps/harbor-shop/backend/harbor-loyalty-backend/requests/contact-updated.json
```

Exercise a second Harbor Shop-specific Rust rule for fulfilment/ops routing:

```bash
curl -sS \
  -X POST http://localhost:8081/api/orders/review \
  -H 'content-type: application/json' \
  --data @apps/harbor-shop/backend/harbor-loyalty-backend/requests/order-review.json
```

## Add Your Own Custom Rule

The shortest safe path is:

1. add request/response types and pure rule logic in `src/lib.rs`
2. add a new handler and route in `src/http.rs`
3. add a sample request in `requests/`
4. add or update `HARBOR_BACKEND_*` env vars in `apps/harbor-shop/docker-compose.yml` if the route needs configuration or secrets
5. add unit tests for the rule and HTTP-level tests for the route

That keeps the service maintainable. Pure rules stay easy to test, and the HTTP adapter stays thin.

The checked-in `review_order(...)` logic in `src/lib.rs` is the example to copy first. The
`POST /api/orders/review` route is only the optional transport wrapper around it.

- `src/lib.rs`
  - `OrderReviewRequest`
  - `OrderReviewResponse`
  - `review_order(...)`
- `src/http.rs`
  - `order_review(...)`
  - `.route("/api/orders/review", post(order_review))`
- `requests/order-review.json`
  - a ready-made payload for local curl testing

That example shows the intended Harbor Shop customization pattern:

- keep the business rule pure and deterministic in Rust
- expose it through the linked crate first
- keep the linked crate ready to implement `davenda-customer-sdk` traits directly
- keep request validation and HTTP concerns in the adapter only when needed
- keep a checked-in sample payload next to the code so another developer can exercise the rule immediately

## When Not To Copy This Pattern

Do not reach for a separate backend process if a smaller boundary is enough.

- If you only need a render hook, small webhook, widget, or bounded integration point, prefer a WASM extension in `apps/harbor-shop/extensions/`.
- If you need customer-owned first-party Rust logic, start with this linked crate.
- If the logic needs deep transactional access, shared platform data ownership, or reuse across many apps, it probably belongs in a native module under `crates/`.

Relevant design docs:

- `docs/design/03-product-shape-core-official-modules-customer-apps.md`
- `docs/design/13-workspace-and-crate-layout.md`
- `docs/design/80-customer-extensions-and-integration-patterns.md`
- `docs/design/96-customer-root-workspaces-and-linked-rust-backends.md`
