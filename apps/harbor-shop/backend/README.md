# Harbor Shop Backend Examples

This folder contains customer-app-owned native Rust backend examples for Harbor Shop.

Use this path when a Harbor Shop customization needs more than templates and does not fit the
bounded WASM-extension model cleanly. Typical reasons:

- the logic needs its own HTTP surface
- the workflow is operationally important enough to want a dedicated process
- the integration needs stricter control over retries, secrets, or outbound traffic

The checked-in example is:

- `harbor-loyalty-backend/`
  - a small Rust HTTP service that exposes Harbor Shop-specific backend logic
  - includes a customer-facing loyalty preview endpoint
  - includes a signed CRM webhook consumer

## Run It

From `apps/harbor-shop`:

```bash
docker compose --profile backend-example up --build
```

That starts the normal Harbor Shop stack plus the backend example on `http://localhost:8081`.

Useful routes:

- `GET http://localhost:8081/`
- `GET http://localhost:8081/health`
- `POST http://localhost:8081/api/loyalty/preview`
- `POST http://localhost:8081/webhooks/crm/contact-updated`

## Local Curl Examples

Preview a customer-specific loyalty decision:

```bash
curl -sS \
  -X POST http://localhost:8081/api/loyalty/preview \
  -H 'content-type: application/json' \
  --data @backend/harbor-loyalty-backend/requests/loyalty-preview.json
```

Exercise the signed webhook path:

```bash
curl -sS \
  -X POST http://localhost:8081/webhooks/crm/contact-updated \
  -H 'content-type: application/json' \
  -H 'x-harbor-backend-secret: harbor-backend-dev-secret' \
  --data @backend/harbor-loyalty-backend/requests/contact-updated.json
```

## What This Example Is Showing

This is not a replacement for Davenda runtime modules. It is an example of customer-owned native
backend work that lives with the customer app instead of being scattered through core.

The example intentionally demonstrates three things:

- Harbor Shop-specific business rules written in plain Rust
- a small HTTP API that another system or frontend can call directly
- a fail-closed signed webhook entrypoint for customer-specific integration work

If you only need a bounded page hook, widget, or lightweight integration point, prefer a WASM
extension. If you need a customer-owned native adapter or service, this is the pattern to follow.
