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

## One Schema, Two Common Files

Typical use:

- `platform.toml`: production-oriented defaults
- `platform.dev.toml`: local or developer-safe overrides

Examples of normal differences:

- cookie `secure = false` in development
- `tls.mode = "external"` locally
- local CDN/object-store endpoints
- development-friendly asset URLs

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

## `[app]`

Supported keys:

- `name`
- `environment`

`environment` values:

- `development`
- `staging`
- `production`

## `[server]`

Supported keys:

- `bind`
- `trusted_proxies`
- `max_body_bytes`

Use this section for network-edge behavior only. It is not where site hosts are declared.

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

### `[http.csrf]`

Supported keys:

- `enabled`
- `field_name`
- `header_name`

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

Not every combination is valid. For example, wildcard certificates require `dns-01`.

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

## `[cache]`

Supported keys:

- `l1`
- `l2`

Supported `l1` values:

- `moka`

Supported `l2` values:

- `redis`
- `valkey`

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
- `sitemap_enabled`

When explicit `[[sites]]` are present, site-level host and locale policy becomes primary, while `[i18n]` and `[seo]` remain the app-wide fallback/default layer.

## `[[sites]]`

Runtime sites mirror the app manifest's site model, but use runtime names:

- `id`
- `display_name`
- `brand_name`
- `canonical_host`
- `hosts`
- `default_locale`
- `supported_locales`
- `localized_routes`

Use this when host resolution, canonical host behavior, or per-site locale routing needs to vary at runtime.

## `[auth]`

Supported keys:

- `package`
- `explain_api`
- `tenant_id`
- `tuple_store_secret`

This section does not define auth semantics. It selects the package and the runtime auth behavior for this deployment.

## `[modules]`

Supported keys:

- `enabled`
- module-owned nested settings via flattened TOML keys

Example:

```toml
[modules]
enabled = ["commerce-payments-stripe"]

[modules."commerce-payments-stripe"]
provider = "stripe"
checkout_mode = "hosted-checkout"
publishable_key = { kind = "env", var = "STRIPE_PUBLISHABLE_KEY" }
webhook_secret = { kind = "env", var = "STRIPE_WEBHOOK_SECRET" }
```

The core contract here is:

- the top-level schema knows there is a `modules` section
- individual module settings stay under that module's own namespace

## `[wasm]`

Supported keys:

- `directory`
- `default_time_limit_ms`
- `allow_network`
- `secret_bindings`
- `outbound_http`

### `[wasm.secret_bindings]`

This maps host-visible secret binding names to `SecretRef` values.

### `[[wasm.outbound_http]]`

Supported keys:

- `integration`
- `endpoint`

## `[jobs]`

Supported keys:

- `backend`
- `retry_limit`

Supported `backend` values:

- `redis`
- `valkey`

## `[observability]`

Supported keys:

- `metrics`
- `tracing`

## `[assets]`

Supported keys:

- `publish_manifest`
- `cdn_base_url`

## Secret References

Several sections use `SecretRef` values instead of raw secrets.

Current forms:

```toml
{ kind = "env", var = "DATABASE_URL" }
{ kind = "secret_manager", provider = "vault", key = "prod/shoppr/database" }
```

Use secret references for:

- database URLs
- object store credentials
- TLS account secrets
- module-specific secrets
- auth tuple-store secrets

## Common Mistakes

- Putting runtime secrets directly into TOML instead of using `SecretRef`.
- Treating `platform.dev.toml` as a separate schema. It should be the same contract with safer development values.
- Mixing site-specific hosts/locales into `[server]` or `[seo]` instead of `[[sites]]`.
- Duplicating module-specific keys at the top level instead of nesting them under `[modules."<module-id>"]`.
- Forgetting that cookie `secure` and TLS mode often need different settings in local development.
- Treating `[auth]` as the place to define relations or capability bindings. That belongs in the auth package, not platform config.
