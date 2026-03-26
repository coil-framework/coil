# Harbor Loyalty Backend

`harbor-loyalty-backend` is the checked-in example of customer-owned native backend logic for Harbor
Shop.

Use it when you need custom Rust code for one customer store and that work does not belong in:

- templates or config
- a bounded WASM extension under `apps/harbor-shop/extensions/`
- a reusable first-party/native module under `crates/`

This example stays intentionally small, but it demonstrates the full path a third-party developer
needs:

- where Harbor Shop-specific Rust code lives
- how that code is exposed over HTTP
- how local Docker wiring starts it
- how signed inbound integration traffic is handled
- how to add tests for the custom logic and the HTTP surface

## What Lives Where

`src/lib.rs`
- pure Harbor Shop business logic and request/response types
- this is where customer-specific rules should start

`src/http.rs`
- the HTTP adapter for this service
- route registration, request validation, and signed webhook checks live here

`src/main.rs`
- process bootstrap
- reads `HARBOR_BACKEND_*` environment variables and starts the Axum server

`requests/*.json`
- checked-in sample payloads for manual `curl` testing

`Dockerfile`
- builds the service for the `backend-example` Docker Compose profile

## How It Connects To Harbor Shop

The connection path is:

1. `apps/harbor-shop/docker-compose.yml` defines the optional `harbor-backend-example` service
2. that service builds this crate and exposes it on `http://localhost:8081`
3. `src/main.rs` loads `HARBOR_BACKEND_BIND`, `HARBOR_BACKEND_BRAND`, and `HARBOR_BACKEND_WEBHOOK_SECRET`
4. `src/http.rs` builds the router and maps routes onto the Harbor Shop business rules in `src/lib.rs`

That is the intended pattern for customer-owned native backend work in Harbor Shop: keep the rules
in app-owned code, keep the process boundary explicit, and avoid scattering the behavior through
Davenda core.

## Run It

From the repo root:

```bash
cargo run --manifest-path apps/harbor-shop/backend/harbor-loyalty-backend/Cargo.toml
```

By default it binds to `0.0.0.0:8081`.

To run it with the full Harbor Shop stack:

```bash
cd apps/harbor-shop
docker compose --profile backend-example up --build
```

Useful routes:

- `GET http://localhost:8081/`
- `GET http://localhost:8081/health`
- `POST http://localhost:8081/api/loyalty/preview`
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

## Add Your Own Custom Rule

The shortest safe path is:

1. add request/response types and pure rule logic in `src/lib.rs`
2. add a new handler and route in `src/http.rs`
3. add a sample request in `requests/`
4. add or update `HARBOR_BACKEND_*` env vars in `apps/harbor-shop/docker-compose.yml` if the route needs configuration or secrets
5. add unit tests for the rule and HTTP-level tests for the route

That keeps the service maintainable. Pure rules stay easy to test, and the HTTP adapter stays thin.

## When Not To Copy This Pattern

Do not reach for a separate backend process if a smaller boundary is enough.

- If you only need a render hook, small webhook, widget, or bounded integration point, prefer a WASM extension in `apps/harbor-shop/extensions/`.
- If the logic needs deep transactional access, shared platform data ownership, or reuse across many apps, it probably belongs in a native module under `crates/`.

Relevant design docs:

- `docs/design/03-product-shape-core-official-modules-customer-apps.md`
- `docs/design/13-workspace-and-crate-layout.md`
- `docs/design/80-customer-extensions-and-integration-patterns.md`
