# Reference Config Schema

**Part:** Appendices  
**Chapter:** 91

The platform configuration surface describes runtime policy, infrastructure bindings, and module activation. It does not store customer content, editorial state, or business records. Those belong in managed data. The examples below use TOML because it maps cleanly to typed Rust settings, but the schema is intended to be format-agnostic.

## Top-Level Sections

| Key | Purpose |
| --- | --- |
| `app` | Application identity, environment, and app-level mode selection |
| `server` | Ports, trusted proxies, body limits, and runtime network behavior |
| `tls` | Certificate provider and termination mode |
| `storage` | Object-store and local-storage policy defaults |
| `cache` | L1 and distributed cache backends, invalidation transport, and cache profiles |
| `i18n` | Locales, fallback chains, default locale, and route localization policy |
| `seo` | Canonical host policy, sitemap behavior, robots defaults, and structured-data toggles |
| `auth` | Auth package selection, tuple storage connection, and explain-mode policy |
| `modules` | Installed official modules and module-specific config namespaces |
| `wasm` | Extension loading policy, resource limits, and allowed host capabilities |
| `jobs` | Queue backends, retry policy, and scheduler settings |
| `observability` | Logging, metrics, tracing, and health endpoint policy |
| `assets` | Build-asset publishing and CDN manifest settings |

## Reference Example

```toml
[app]
name = "showcase-events"
environment = "production"

[server]
bind = "0.0.0.0:8080"
trusted_proxies = ["10.0.0.0/8"]

[tls]
mode = "acme"
challenge = "dns-01"
provider = "cloudflare-dns"

[storage]
default_class = "public_upload"
object_store = "s3"
local_root = "/var/lib/platform"

[cache]
l1 = "moka"
l2 = "redis"

[i18n]
default_locale = "en-GB"
supported_locales = ["en-GB", "fr-FR"]
fallback_locale = "en-GB"

[seo]
canonical_host = "www.example.com"
emit_json_ld = true

[auth]
package = "platform-default-auth"
explain_api = false

[modules]
enabled = [
  "cms-pages",
  "admin-shell",
  "memberships",
  "events",
  "media-library",
]

[wasm]
directory = "extensions"
default_time_limit_ms = 50
allow_network = false

[jobs]
backend = "redis"

[observability]
metrics = true
tracing = true

[assets]
publish_manifest = true
cdn_base_url = "https://cdn.example.com"
```

## Schema Rules

The following rules keep config maintainable:

- secrets are referenced indirectly through environment variables or a secret provider, never committed as plain values in the main config file
- customer content such as page copy, SEO text, or editorial metadata does not belong in config
- module namespaces may add fields, but they must remain under the owning module key rather than polluting the top level
- deprecated keys should remain readable with warnings until the next major release

## Boundary Between Config And Data

Runtime policy belongs in config. Customer-specific content belongs in data.

- “Use ACME with DNS-01 and Cloudflare DNS automation” is config.
- “This event page has French SEO metadata and a published hero image” is data.
- “The storefront supports `en-GB` and `fr-FR`” is config.
- “This product’s translated description and per-locale slug” is data.

That boundary matters because config is loaded at boot and reviewed operationally, while content is edited, published, cached, and migrated through module workflows.
