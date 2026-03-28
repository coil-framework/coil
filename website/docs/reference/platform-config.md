---
title: platform.toml And platform.dev.toml
---

`platform.toml` and `platform.dev.toml` are runtime configuration files.

They describe:

- environment and server policy
- HTTP session, flash, and CSRF behavior
- TLS mode
- database, storage, cache, jobs, and observability
- site host bindings at runtime
- auth runtime settings
- official module runtime settings
- WASM loading policy
- asset publication settings

`platform.dev.toml` is usually a development-safe variant of the same schema. It is not a different model.

## Why These Files Exist

Davenda keeps runtime operations separate from product composition.

- `app.toml` says what the app is
- `platform.toml` says how it runs in one environment

That lets the same customer app move across local development, staging, and production without rewriting the app manifest.

In practice:

- `app.toml` changes when the product changes
- `platform.dev.toml` changes when your local environment changes
- `platform.toml` changes when your production infrastructure or operating policy changes

## Shoppr As The Working Example

The checked-in Shoppr platform config files are the best current concrete examples:

- `apps/shoppr/platform.dev.toml`
- `apps/shoppr/platform.toml`

Keep those files open while reading this page. They show the same product running with two different runtime policies:

- local development on plain HTTP with development-safe cookies and a local CDN/object-store URL
- production-oriented TLS, secure cookies, a production local root, and a production CDN base URL

## One Schema, Two Common Files

Typical use:

- `platform.toml`: production-oriented defaults
- `platform.dev.toml`: local or developer-safe overrides

Examples of normal differences:

- cookie `secure = false` in development
- `tls.mode = "external"` locally
- local CDN/object-store endpoints
- development-friendly asset URLs

The point is not to maintain two unrelated files. The point is to keep one runtime schema and two environment-shaped realizations of it.

## Top-Level Sections

The current platform config loader supports:

- `[app]`
- `[server]`
- `[http.session]`
- `[http.session_cookie]`
- `[http.flash_cookie]`
- `[http.csrf]`
- `[tls]`
- `[database]`
- `[storage]`
- `[cache]`
- `[i18n]`
- `[seo]`
- `[[sites]]`
- `[auth]`
- `[modules]`
- `[wasm]`
- `[wasm.secret_bindings]`
- `[[wasm.outbound_http]]`
- `[jobs]`
- `[observability]`
- `[assets]`

Not every app needs every block, but most production apps will use most of them.

## Reference Example

```toml
[app]
name = "shoppr"
environment = "production"

[server]
bind = "0.0.0.0:8080"
trusted_proxies = ["10.0.0.0/8"]

[http.session]
store = "redis"
idle_timeout_secs = 3600
absolute_timeout_secs = 86400

[http.session_cookie]
name = "davenda_session"
path = "/"
same_site = "lax"
secure = true
http_only = true

[http.flash_cookie]
name = "davenda_flash"
path = "/"
same_site = "lax"
secure = true
http_only = true

[http.csrf]
enabled = true
field_name = "_csrf"
header_name = "x-csrf-token"

[tls]
mode = "acme"
challenge = "dns-01"
provider = "cloudflare-dns"

[database]
url = { kind = "env", var = "DATABASE_URL" }
schema = "public"

[storage]
default_class = "public_upload"
deployment = "distributed"
object_store = "s3"
object_store_secret = { kind = "env", var = "OBJECT_STORE_URL" }
local_root = "/var/lib/davenda/shoppr"

[cache]
l1 = "moka"
l2 = "redis"

[auth]
package = "shoppr-auth"
explain_api = false
tenant_id = 101
```

## A Practical Reading Strategy

Read platform config in this order:

1. What environment am I in?
2. How do requests enter the runtime?
3. How are sessions and CSRF enforced?
4. How does the runtime reach data, storage, cache, and jobs?
5. How is TLS handled?
6. How are sites and canonical hosts represented at runtime?
7. Which module-specific settings exist?
8. Where are assets published and served from?

That is usually the same order you debug it in too.

## `[app]`

Supported keys:

- `name`
- `environment`

`environment` values:

- `development`
- `staging`
- `production`

### What This Block Means

This is the runtime identity and environment mode.

`name` should match the app manifest. `environment` changes how the runtime interprets safety-sensitive behaviour such as local HTTP object-store access and other development allowances.

### Example

```toml
[app]
name = "shoppr"
environment = "development"
```

### Guidance

- `development` should be the normal local mode
- `production` should be used only when the surrounding infra is production-shaped
- do not use production mode locally unless you deliberately want production-like restrictions

## `[server]`

Supported keys:

- `bind`
- `trusted_proxies`
- `max_body_bytes`

### What This Block Means

This is the transport-edge block for the HTTP server. It controls:

- where the server listens
- which upstream proxies are trusted to supply forwarded metadata
- request body size limits

### Example

```toml
[server]
bind = "0.0.0.0:8080"
trusted_proxies = ["10.0.0.0/8"]
max_body_bytes = 10485760
```

### Guidance

- use `bind` for local or container network binding, not for public hostname modelling
- use `trusted_proxies` only for real proxy networks you control
- keep body limits explicit for upload-heavy apps

## `[http.*]`

HTTP config is split into four typed sections.

### `[http.session]`

Supported keys:

- `store`
- `idle_timeout_secs`
- `absolute_timeout_secs`

Supported `store` values:

- `memory`
- `database`
- `redis`
- `valkey`

### What This Block Means

This controls how browser sessions are persisted and how long they remain valid.

### Example

```toml
[http.session]
store = "redis"
idle_timeout_secs = 3600
absolute_timeout_secs = 86400
```

### Guidance

- `memory` is only for local or explicitly single-node use
- `database`, `redis`, or `valkey` are the real shared-store options
- `idle_timeout_secs` is inactivity-based
- `absolute_timeout_secs` is hard-stop lifetime

### `[http.session_cookie]` and `[http.flash_cookie]`

Supported keys:

- `name`
- `domain`
- `path`
- `same_site`
- `secure`
- `http_only`
- `protection`

Supported `same_site` values:

- `lax`
- `strict`
- `none`

Supported `protection` values:

- `signed`
- `encrypted`

### What These Blocks Mean

These blocks define the cookie transport behaviour for sessions and flash state.

### Example

```toml
[http.session_cookie]
name = "davenda_session"
path = "/"
same_site = "lax"
secure = true
http_only = true
protection = "encrypted"
```

### Guidance

- `secure = false` is normal in local plain-HTTP development
- `secure = true` should be the production default
- use `encrypted` when the cookie value should not be readable client-side
- keep cookie naming stable across deployments unless you are deliberately rotating transport state

### `[http.csrf]`

Supported keys:

- `enabled`
- `field_name`
- `header_name`

### What This Block Means

This enables and names CSRF transport channels for state-changing browser requests.

### Example

```toml
[http.csrf]
enabled = true
field_name = "_csrf"
header_name = "x-csrf-token"
```

### Guidance

- leave it enabled for normal browser apps
- use the documented field and header names in forms and enhanced requests

## `[tls]`

Supported keys:

- `mode`
- `challenge`
- `provider`
- `account_secret`

Supported `mode` values:

- `external`
- `acme`
- `cloudflare-origin`
- `manual`

Supported `challenge` values:

- `http-01`
- `tls-alpn-01`
- `dns-01`

Supported `provider` values:

- `cloudflare-dns`
- `cloudflare-origin-ca`
- `manual-import`

### What This Block Means

This is the TLS ownership and issuance block. It tells Davenda whether:

- TLS is handled outside Davenda
- Davenda should obtain certificates
- Davenda should validate origin-only certificates
- certificates are manually imported

### Guidance

- use `external` in local development or when TLS is terminated elsewhere
- use `acme` for normal public certificate management
- use `cloudflare-origin` only when the origin is intentionally private behind Cloudflare
- use `dns-01` for wildcard-heavy or CDN-fronted deployments

## `[database]`

Supported keys:

- `driver`
- `url`
- `schema`
- `migrations_table`
- `min_connections`
- `max_connections`
- `statement_timeout_secs`

Supported `driver` values:

- `postgres`

The URL is a `SecretRef`, typically:

```toml
url = { kind = "env", var = "DATABASE_URL" }
```

### What This Block Means

This is the runtime database connection contract for:

- application data
- migration ownership
- shared runtime surfaces that rely on database connectivity

### Guidance

- Postgres is the production-grade path
- set connection pool sizes deliberately
- keep `migrations_table` stable once a deployment is in use
- use `statement_timeout_secs` to bound runaway queries

## `[storage]`

Supported keys:

- `default_class`
- `deployment`
- `single_node_escape_hatch`
- `object_store`
- `local_root`
- `object_store_secret`

Supported `default_class` values:

- `public_asset`
- `public_upload`
- `private_shared`
- `local_only_sensitive`

Supported `deployment` values:

- `distributed`
- `single_node`

Supported `single_node_escape_hatch` values:

- `disabled`
- `explicit_single_node`

Supported `object_store` values:

- `s3`

### What This Block Means

This block defines the storage topology and delivery posture for assets and uploads.

### Guidance

- use `distributed` when the app is meant to scale beyond one node
- treat `single_node_escape_hatch` as an explicit exception, not a default
- `local_root` still matters even in distributed setups because some local-only classes remain intentionally local
- `object_store_secret` should resolve to the structured secret shape Davenda expects

## `[cache]`

Supported keys:

- `l1`
- `l2`

Supported `l1` values:

- `moka`

Supported `l2` values:

- `redis`
- `valkey`

### What This Block Means

Davenda uses a two-level cache vocabulary:

- `l1`: in-process cache close to the runtime instance
- `l2`: shared cache across instances

### Guidance

- `l1` is the fast local layer
- `l2` is the shared coordination layer
- a distributed production deployment should normally have both
- a small local development setup can still feel fine with the same shape because Shoppr uses `moka` plus `redis`

If you are asking "which one is required?", the real answer is:

- single-process local development can survive with less
- production distributed deployments should treat `l2` as part of the real contract

## `[i18n]` and `[seo]`

These sections provide app-wide runtime defaults and compatibility sugar.

### `[i18n]`

Supported keys:

- `default_locale`
- `supported_locales`
- `fallback_locale`
- `localized_routes`

### `[seo]`

Supported keys:

- `canonical_host`
- `emit_json_ld`

### Guidance

These blocks should align with the product contract expressed in `app.toml`. They are runtime defaults and validation context, not a replacement for the app manifest.

## `[[sites]]`

Supported keys:

- `id`
- `display_name`
- `brand_name`
- `canonical_host`
- `hosts`
- `default_locale`
- `supported_locales`

### What This Block Means

This is the runtime host-resolution view of multi-site configuration.

Use it to tell the runtime:

- which hostnames map to which site
- which locale defaults apply at runtime
- which brand display strings should be surfaced per site

### Guidance

- this block should stay aligned with `app.toml`
- runtime hostnames here are operational host bindings, not just product declarations
- keep host coverage explicit; unknown hosts should fail closed

## `[auth]`

Supported keys:

- `package`
- `explain_api`
- `tenant_id`

### What This Block Means

This is the runtime binding for the selected auth package and related auth runtime behaviour.

### Guidance

- `package` should match the app manifest selection
- `tenant_id` should be stable per deployment
- `explain_api` should only be enabled deliberately when you want that operational surface available

## `[modules]`

`[modules]` mirrors app-level intent but allows module-specific runtime config blocks.

### Example

```toml
[modules]
enabled = ["commerce", "commerce-payments-stripe"]

[modules."commerce-payments-stripe"]
provider = "stripe"
checkout_mode = "hosted-checkout"
publishable_key = { kind = "env", var = "STRIPE_PUBLISHABLE_KEY" }
webhook_secret = { kind = "env", var = "STRIPE_WEBHOOK_SECRET" }
```

### Guidance

Think of this as the runtime wiring layer for installed modules. The app manifest chooses the product battery; platform config supplies runtime secrets and provider settings for that battery.

## `[wasm]`, `[wasm.secret_bindings]`, and `[[wasm.outbound_http]]`

These sections control the runtime-only extension host.

### `[wasm]` supported keys

- `directory`
- `default_time_limit_ms`
- `allow_network`

### `[wasm.secret_bindings]`

Maps named extension-visible secret bindings to platform secrets.

### `[[wasm.outbound_http]]`

Declares explicitly allowed outbound HTTP endpoints for extensions.

### Guidance

- keep the extension directory explicit
- prefer deny-by-default for network access
- make secret bindings narrow and named
- use explicit endpoint mappings instead of ambient outbound access

## `[jobs]`

Supported keys:

- `backend`

Typical values include:

- `redis`
- `valkey`

### What This Block Means

This selects the shared backend used for queues, leases, scheduled work, and recovery operations.

### Guidance

Use the same shared backend story as the rest of the deployment. If the app is distributed, jobs should be too.

## `[observability]`

Supported keys:

- `metrics`
- `tracing`

### What This Block Means

This toggles baseline runtime observability surfaces.

### Guidance

These are not abstract nice-to-haves. They are the runtime switches that determine whether the platform emits the signals the operations docs rely on.

Read next:

- [Observability, monitoring, and audit](../operations/observability.md)

## `[assets]`

Supported keys:

- `publish_manifest`
- `cdn_base_url`

### What This Block Means

This block controls how published theme assets are surfaced after publication.

### Guidance

`cdn_base_url` is not inherently required in the abstract, but if `publish_manifest` is enabled the runtime needs a stable delivery base to point published asset URLs at.

That base can be:

- a true CDN domain
- an object-store-backed delivery domain
- a same-domain asset host if your production topology is intentionally set up that way

The important thing is not "must this be a CDN?" The important thing is "is there a stable, production-valid base URL for published assets?"

Shoppr demonstrates both:

- local development using `http://localhost:9000/shoppr`
- production using `https://cdn.example.com`

## Common Configuration Decisions

### Can I serve production assets from my main site instead of a CDN?

Yes, if your production topology is intentionally built that way and the delivery URL is stable. Davenda does not require a third-party CDN brand name. It requires a reliable published-asset base URL.

### Should I use both `l1` and `l2` cache?

For distributed production systems, yes. `l1` gives process-local speed; `l2` gives cross-instance coordination.

### Should local development mirror production shape?

Broadly yes, but with development-safe differences:

- plain HTTP
- insecure cookies if needed locally
- local delivery endpoints
- externally terminated TLS mode

## Common Mistakes

- Putting product configuration into platform config instead of `app.toml`
- Running production mode locally and then treating local failures as product problems
- Treating `trusted_proxies` as a convenience wildcard
- Using in-memory sessions for a deployment that expects shared state
- Forgetting that module runtime blocks are separate from app manifest module enablement
- Treating `cdn_base_url` as a branding choice instead of a delivery contract

## Read Next

- [app.toml](app-toml.md)
- [Build and deploy](../operations/build-and-deploy.md)
- [Configuration and secrets](../operations/configuration-and-secrets.md)
- [Cache, TLS, cutover, and rollback](../operations/cache-tls-cutover-and-rollback.md)
