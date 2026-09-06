---
title: Configuration And Secrets
---

Coil separates customer composition, operational configuration, and secrets on purpose.

That split is one of the platform's core safety properties.

## What Is This?

This page explains:

- what belongs in the app manifest
- what belongs in `platform.dev.toml` and `platform.toml`
- what belongs in secret storage or environment variables
- how to reason about a complete configuration set

## Why Does This Separation Exist?

Without it, teams usually end up with one giant config surface that mixes:

- product shape
- environment topology
- credentials
- temporary operational workarounds

That makes review harder, drift easier, and incident recovery slower.

## The Three Configuration Inputs

### 1. Customer app manifest

The app manifest describes product composition:

- app identity
- enabled modules
- site and locale structure
- theme settings
- auth package identity
- runtime-installed extension declarations

### 2. Platform runtime config

Platform config describes how the product is operated in a specific environment:

- bind address
- database, cache, jobs, and storage backends
- TLS mode
- observability settings
- asset delivery settings
- module-specific operational config

### 3. Secrets

Secrets provide sensitive runtime values:

- database URLs
- object-store credentials
- payment API keys
- webhook secrets
- TLS provider credentials

## A Canonical Coil Example

This is the mental model you should start from:

```toml
[database]
url = { kind = "env", var = "DATABASE_URL" }
schema = "public"

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
cdn_base_url = "https://cdn.example.com"
```

What this example demonstrates:

- topology lives in platform config
- sensitive values resolve from environment or secret storage
- product structure does not live here unless it is truly operational

## How To Think About The Important Blocks

### `[server]`

Use it for:

- bind address
- trusted proxies

Do not use it for product identity, site catalog policy, or theme behaviour.

### `[database]`

Use it for:

- connection URL reference
- schema
- pool sizing
- statement timeouts

The secret value for the URL should still come from a secret source.

### `[storage]`

Use it for:

- object store type
- local root
- deployment mode
- secret reference for credentials

### `[cache]`

Use it for:

- `l1` in-process cache
- `l2` shared cache such as Redis or Valkey

This is an operational performance choice, not an app-manifest concern.

### `[i18n]` and `[[sites]]`

Use them for:

- default and supported locales
- route localisation policy
- host and canonical host mapping

These are runtime-facing because they affect routing and delivery, even though they are also part
of product behaviour.

### `[auth]`

Use it for:

- auth package identity
- auth backend wiring such as tenant id where applicable

Do not place app-specific capability semantics here if they belong in auth package files.

### `[modules]`

Use it for:

- enabled linked official modules
- module-specific operational configuration

Example:

```toml
[modules."commerce-payments-stripe"]
provider = "stripe"
checkout_mode = "hosted-checkout"
publishable_key = { kind = "env", var = "STRIPE_PUBLISHABLE_KEY" }
webhook_secret = { kind = "env", var = "STRIPE_WEBHOOK_SECRET" }
```

### `[wasm]`

Use it for:

- extension artifact directory
- runtime limits
- secret bindings exposed to extensions

### `[jobs]`

Use it for:

- queue backend selection

### `[observability]`

Use it for:

- metrics
- tracing

### `[assets]`

Use it for:

- whether publication emits an asset manifest
- the asset origin or CDN base URL

## Development Versus Production

Development and production do not need identical infrastructure, but they should preserve the same
behavioural model.

Good differences:

- `secure = false` for dev cookies and `true` for production
- local object-store/CDN URLs in development and real ones in production
- local `tls.mode = "external"` in development and real TLS automation in production

Bad differences:

- different module sets
- different site topology
- different auth package identity
- different route graph

## A Practical Local Secret Example

For local development, a small `.env` can be enough:

```dotenv
DATABASE_URL=postgres://coil:coil@127.0.0.1:5432/coil_app
OBJECT_STORE_URL=...
STRIPE_PUBLISHABLE_KEY=pk_test_replace_me
STRIPE_SECRET_KEY=sk_test_replace_me
STRIPE_WEBHOOK_SECRET=whsec_replace_me
```

The exact variable names vary by app and provider, but the rule stays the same:

- committed config references the secret
- the secret value arrives from the environment

## Secrets Handling Rules

Prefer:

- environment variables
- deployment-time secret injection
- a secret manager exposed through the runtime environment

Avoid:

- committing provider keys
- copying production secrets into local config files
- embedding secrets in templates or frontend assets

## Supporting Repo Examples

After you understand the pattern, the checked-in apps are useful supporting examples:

- Shoppr shows payment-provider and webhook-heavy config
- Gitly shows a simpler non-commerce config shape

Use those examples as proofs of the model, not as the primary teaching material.

## Common Mistakes

### Putting product behaviour in `platform.toml`

Product composition belongs in the app manifest unless it is genuinely an operational concern.

### Putting credentials in committed config

Secrets belong in secret resolution, not in versioned product files.

### Making development config too fake

Development can be lighter than production, but it should still exercise the same integration
boundaries.

## What To Read Next

- [Build and deploy](../build-and-deploy/)
- [Asset publication and CDN delivery](../asset-publication-and-cdn-delivery/)
- [Webhooks and integrations](../webhooks-and-integrations/)
