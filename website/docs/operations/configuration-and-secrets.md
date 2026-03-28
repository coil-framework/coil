---
title: Configuration And Secrets
---

Davenda separates customer composition, operational configuration, and secrets on purpose.

That split is one of the platform's core safety properties.

## What Is This?

This page explains:

- what belongs in the app manifest
- what belongs in `platform.dev.toml` and `platform.toml`
- what belongs in secret storage or environment variables
- how the checked-in apps use those boundaries

## Why Does This Separation Exist?

Without it, teams usually end up with one giant config surface that mixes:

- product shape
- environment topology
- credentials
- temporary operational workarounds

That makes review harder, drift easier, and incident recovery slower.

## The Three Configuration Inputs

### 1. Customer app manifest

The app manifest describes product composition.

Typical concerns:

- app identity
- installed modules
- site and locale policy
- theme settings
- auth package identity
- runtime-installed extension declarations

Concrete files:

- `apps/shoppr/app.toml`
- `apps/gitly/app.toml`

### 2. Platform runtime config

Platform config describes how the product is operated in a specific environment.

Typical concerns:

- bind address
- database, cache, jobs, and storage backends
- TLS mode
- observability settings
- asset publication settings
- payment provider wiring

Concrete files:

- `apps/shoppr/platform.dev.toml`
- `apps/shoppr/platform.toml`
- `apps/gitly/platform.dev.toml`
- `apps/gitly/platform.toml`

### 3. Secrets

Secrets provide sensitive runtime values.

Typical concerns:

- database URL
- object store credentials
- payment API keys
- webhook secrets
- TLS provider credentials

Concrete checked-in examples:

- `apps/shoppr/.env.example`
- `apps/gitly/.env.example`

## A Concrete Davenda Config Example

From Shoppr development config:

```toml
[database]
url = { kind = "env", var = "DATABASE_URL" }

[cache]
l1 = "moka"
l2 = "redis"

[jobs]
backend = "redis"

[observability]
metrics = true
tracing = true

[assets]
publish_manifest = true
cdn_base_url = "http://localhost:9000/shoppr"
```

That is the Davenda pattern:

- operational topology in platform config
- secrets resolved through env bindings
- site and module composition kept elsewhere

## Full Development Example

Shoppr's current development environment uses:

- `platform.dev.toml` for local-safe server and cookie settings
- `.env.example` for provider env var names
- Docker Compose for Postgres, Redis, and object storage

Key local secret variables in Shoppr:

```dotenv
STRIPE_PUBLISHABLE_KEY=pk_test_replace_me
STRIPE_SECRET_KEY=sk_test_replace_me
STRIPE_WEBHOOK_SECRET=whsec_replace_me
HARBOR_BACKEND_WEBHOOK_SECRET=harbor-backend-dev-secret
```

Gitly shows a different kind of local config emphasis. Its `.env.example` is mostly local port and
compose wiring because the checked-in app does not need the same payment secrets.

## How To Think About The Important Blocks

### `[server]`

Use it for:

- bind address
- trusted proxies

Do not use it for product identity or site configuration.

### `[database]`

Use it for:

- connection URL
- schema
- connection pool sizing
- statement timeouts

The database URL itself should come from secret resolution.

### `[storage]`

Use it for:

- object store type
- local root
- deployment mode
- the secret reference that resolves object store credentials

### `[cache]`

Use it for:

- `l1` in-process caching
- `l2` shared Redis or Valkey caching

This is an operational performance choice, not a template or app-manifest concern.

### `[i18n]` and `[[sites]]`

Use them for:

- default locale
- supported locales
- route localization policy
- host and canonical host mapping per site

Shoppr demonstrates the multi-site version of this model. Gitly demonstrates the single-site but
multi-locale version.

### `[auth]`

Use it for:

- auth package identity
- tenant or environment wiring relevant to the auth backend

Do not place app-specific capability semantics here if they belong in the auth package files.

### `[modules]`

Use it for:

- enabling linked official modules
- module-specific operational config blocks

Shoppr's Stripe configuration lives under:

- `[modules."commerce-payments-stripe"]`

### `[wasm]`

Use it for:

- extension artifact directory
- runtime limits
- secret bindings exposed to extensions

### `[jobs]`

Use it for:

- queue backend selection

Current checked-in examples use Redis-backed jobs.

### `[observability]`

Use it for:

- enabling metrics
- enabling tracing

### `[assets]`

Use it for:

- whether publication emits an asset manifest
- where assets should be served from

## Development Versus Production

The development and production files do not need identical infrastructure, but they should preserve
the same behavioral model.

Good differences:

- cookie `secure = false` in development and `true` in production
- local object-store/CDN URLs in development and real CDN URLs in production
- local TLS mode of `external` in dev and real TLS mode in production

Bad differences:

- different module sets
- different site topology
- different auth package identity
- different route graph

## Secrets Handling Rules

Prefer:

- environment variables
- deployment-time secret injection
- explicit secret manager integration exposed through runtime env

Avoid:

- committing provider keys
- copying production secrets into local config files
- embedding secrets in templates or frontend assets

## Common Mistakes

### Putting product behavior in `platform.toml`

Product composition belongs in the app manifest, not in runtime env config.

### Putting credentials in the app manifest

Secret values belong in secret resolution, not in committed product files.

### Making development config too fake

Development config can be lighter than production, but it should still exercise the same
behavioral integration boundaries.

## What To Read Next

- [Build and deploy](build-and-deploy.md)
- [Asset publication and CDN delivery](asset-publication-and-cdn-delivery.md)
- [Webhooks and integrations](webhooks-and-integrations.md)
