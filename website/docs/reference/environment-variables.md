---
title: Environment Variables
---

Davenda does not treat environment variables as a hidden side channel. The runtime reads them
through explicit secret references in platform config and customer bootstrap scripts.

Use this page when you want to answer:

- which env vars the demos actually need
- where they are declared in config
- which ones are platform-wide versus app-specific

## How Davenda Resolves Environment Secrets

The runtime resolver lives in `crates/davenda-runtime/src/server/backend.rs`.

`EnvironmentSecretResolver` supports:

- `SecretRef::Env { var }`

and intentionally rejects unavailable sources in the live runtime boundary.

This matters because the config files are the source of truth. The environment variable names are
declared there, not guessed inside random helper code.

## Platform-Wide Variables You Will See Often

These names appear across the checked-in demos and runtime tests:

- `DAVENDA_CONFIG`
  - config file path used by app entrypoints and container bootstrap
- `DAVENDA_BIND`
  - optional bind override used by runtime serve helpers
- `DAVENDA_COOKIE_SECRET`
  - session cookie secret
- `DAVENDA_CSRF_SECRET`
  - CSRF signing secret
- `DATABASE_URL`
  - database connection string
- `REDIS_URL`
  - Redis backend connection string
- `OBJECT_STORE_URL`
  - object-store credential/config payload

Concrete config references:

- `apps/shoppr/platform.dev.toml`
- `apps/gitly/platform.dev.toml`

## Shoppr-Specific Variables

Shoppr’s local template is `apps/shoppr/.env.example`.

Current documented variables:

- `STRIPE_PUBLISHABLE_KEY`
- `STRIPE_SECRET_KEY`
- `STRIPE_WEBHOOK_SECRET`
- `HARBOR_BACKEND_WEBHOOK_SECRET`

Where they are consumed:

- `apps/shoppr/platform.dev.toml`
  - Stripe payment module config and WASM secret binding
- `apps/shoppr/docker-compose.yml`
  - local container wiring
- `apps/shoppr/docker/entrypoint.sh`
  - startup warnings and bootstrap flow
- `apps/shoppr/backend/shoppr-loyalty-backend/src/main.rs`
  - optional sidecar secrets and bind settings

## Gitly-Specific Variables

Gitly’s local template is `apps/gitly/.env.example`.

Current variables there are mostly host-port overrides:

- `COMPOSE_PROJECT_NAME`
- `GITLY_HTTP_PORT`
- `GITLY_POSTGRES_PORT`
- `GITLY_REDIS_PORT`
- `GITLY_MINIO_PORT`
- `GITLY_MINIO_CONSOLE_PORT`

Gitly’s runtime secrets are still declared in:

- `apps/gitly/platform.dev.toml`
- `apps/gitly/docker-compose.yml`

The key operational vars remain:

- `DATABASE_URL`
- `REDIS_URL`
- `OBJECT_STORE_URL`
- `DAVENDA_COOKIE_SECRET`
- `DAVENDA_CSRF_SECRET`

## `OBJECT_STORE_URL`

This is the least obvious variable, so it is worth calling out directly.

It is not a single bare URL. The demos use a structured object-store config payload, for example
through Docker Compose values in:

- `apps/shoppr/docker-compose.yml`
- `apps/gitly/docker-compose.yml`

The runtime then parses it through the storage/backend layer in:

- `crates/davenda-runtime/src/server/backend.rs`

## App Root Variables In Customer Workspaces

The demo customer apps also support workspace-root discovery overrides:

- Shoppr:
  - `HARBOUR_SHOP_APP_ROOT`
  - `HARBOR_SHOP_APP_ROOT`
- Gitly:
  - `GITLY_APP_ROOT`

Concrete files:

- `apps/shoppr/crates/shoppr-app/src/lib.rs`
- `apps/gitly/crates/gitly-app/src/lib.rs`

These exist to make the customer binaries usable from more than one launch context.

## What To Put In `.env.example`

The checked-in demos use a pragmatic split:

- secrets or placeholders the developer is expected to override go in `.env.example`
- env-backed secret names are still declared in `platform.dev.toml`
- Docker Compose passes them through into the app container

That is the pattern to copy for your own customer app.

## Common Mistakes

- Do not invent a variable name in code without declaring it in config.
- Do not put app secrets only in README prose.
- Do not assume `OBJECT_STORE_URL` is interchangeable with a plain S3 URL string.
- Do not forget that the customer binary and container bootstrap may also rely on
  `DAVENDA_CONFIG` or app-root overrides.

## Read Next

- [Platform Config](./platform-config.md)
- [CLI Commands](./cli-commands.md)
- [Gitly Build And Deploy](../use-cases/gitly/build-and-deploy.md)
