---
title: Environment Variables
---

Davenda uses environment variables for runtime secrets, deployment-specific addresses, and selected
operator controls.

This page gathers the environment variables that appear across the public runtime and demo apps.

## Core Runtime Variables

### `DAVENDA_CONFIG`

Purpose:

- selects the platform config path when the CLI or runtime cannot rely on discovery

Used by:

- CLI argument parsing
- customer-root bootstrap

### `DAVENDA_BIND`

Purpose:

- overrides the HTTP bind address for runtime serving

Used by:

- runtime `serve_from_env()` paths
- `davenda-all`

### `DAVENDA_COOKIE_SECRET`

Purpose:

- session cookie signing and encryption secret

Required for:

- normal browser session handling

### `DAVENDA_CSRF_SECRET`

Purpose:

- CSRF token protection secret

Required for:

- mutating browser flows and protected forms

### `DAVENDA_SHARED_STATE_DIR`

Purpose:

- shared local state directory for runtime state that is not stored in the primary database

## Database And Queue Variables

### `DATABASE_URL`

Purpose:

- primary database connection string for distributed runtime state, data, and jobs coordination

Used by:

- platform database config
- jobs inspection and ready/dead-letter commands
- browser session backends that require distributed state

### `REDIS_URL`

Purpose:

- Redis-backed distributed cache

Required when:

- platform config selects Redis cache

### `VALKEY_URL`

Purpose:

- Valkey-backed distributed cache

Required when:

- platform config selects Valkey cache and `REDIS_URL` is not provided

### `DAVENDA_SHARED_BACKEND_NAMESPACE`

Purpose:

- shared jobs backend namespace

Used by:

- jobs shared backend

## Storage And Asset Variables

### `OBJECT_STORE_URL`

Purpose:

- object-store connection secret used by Shoppr and Gitly platform configs

Used by:

- published theme assets
- managed media
- storage verification

## TLS And Cutover Variables

### `DAVENDA_TLS_MATERIAL_KEY`

Purpose:

- encrypts TLS certificate material managed by the platform

### `DAVENDA_TLS_PREVIOUS_MATERIAL_KEYS`

Purpose:

- previous key material kept during rotation

### `DAVENDA_TLS_STATE_DIR`

Purpose:

- testing and local TLS state directory override

### `DAVENDA_CLOUDFLARE_API_BASE_URL`

Purpose:

- Cloudflare API base URL override for cutover testing or special control-plane setups

### `DAVENDA_CUTOVER_CLOUDFLARE_SECRET`

Purpose:

- Cloudflare DNS switch credential fallback when not supplied through config secrets

### `DAVENDA_CUTOVER_ALLOW_SYNTHETIC_SESSION`

Purpose:

- enables synthetic-session cutover probes where that mode is explicitly supported

## WASM And Host Runtime Variables

### `DAVENDA_WASM_HTTP_NO_FALLBACK`

Purpose:

- host testing and control over WASM HTTP fallback behaviour

## Demo-Specific Variables

### Shoppr

- `HARBOR_BACKEND_BIND`
- `HARBOR_BACKEND_BRAND`
- `HARBOR_BACKEND_WEBHOOK_SECRET`
- `HARBOR_SHOP_APP_ROOT`
- `HARBOUR_SHOP_APP_ROOT`

These exist because Shoppr evolved from the earlier Harbour naming and still carries a compatibility
layer for app-root discovery and the side example backend.

### Gitly

- `OCTOHUB_APP_ROOT`

This remains the compatibility app-root variable for the renamed Gitly demo.

## How To Use This Page

When you add a new environment variable to customer code or platform code:

1. add it to the config or runtime implementation
2. document it here
3. document its operational use in the relevant operations page
4. add it to Shoppr or Gitly examples if it belongs in a canonical workflow

## Common Mistakes

- Putting product behaviour in environment variables instead of app or platform config.
- Forgetting to rotate secrets like `DAVENDA_COOKIE_SECRET`, `DAVENDA_CSRF_SECRET`, or TLS
  material keys.
- Assuming `DATABASE_URL` is only for data and not also used by jobs and session surfaces.

## Read Next

- [Platform Config](./platform-config.md)
- [Configuration And Secrets](../operations/configuration-and-secrets.md)
- [CLI Commands](./cli-commands.md)
