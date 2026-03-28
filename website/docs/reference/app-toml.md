---
title: app.toml
---

`app.toml` is the customer app manifest.

It describes the product-facing shape of an app:

- app identity
- domains and sites
- locale policy
- theme/template roots
- auth package selection
- installed official modules
- customer-owned content model and migration declarations

It is not the place for infrastructure secrets, runtime connection strings, or deployment-only tuning. Those belong in `platform.toml` or `platform.dev.toml`.

## Why This File Exists

Davenda separates product composition from runtime operations.

- `app.toml` answers "what kind of app is this?"
- `platform.toml` answers "how does this app run here?"

That split matters because a customer app should be able to keep its product contract stable while moving between development, staging, and production environments.

## Supported Top-Level Sections

The current manifest loader supports these top-level sections:

- `[app]`
- `[domains]`
- `[i18n]`
- `[[sites]]`
- `[theme]`
- `[auth]`
- `[modules]`
- `[[content_models]]`
- `[[customer_migrations]]`

## Minimal Example

```toml
[app]
name = "shoppr"
display_name = "Shoppr"

[domains]
canonical = "shop.example.com"
additional = ["www.example.com"]

[i18n]
default_locale = "en-GB"
supported_locales = ["en-GB", "fr-FR"]
localized_routes = true

[theme]
active = "shoppr"
template_namespaces = ["customer-app", "shoppr"]
asset_roots = ["theme/assets"]

[auth]
mode = "extend"
package = "shoppr-auth"

[modules]
enabled = ["cms", "commerce", "admin"]
```

## `[app]`

`[app]` defines the customer app identity.

Supported keys:

- `name`
- `display_name`

Notes:

- `name` is the stable app id used across composition and runtime contracts.
- `display_name` is human-readable and falls back to `name` if omitted.

## `[domains]`

`[domains]` is the app-level compatibility domain block.

Supported keys:

- `canonical`
- `additional`

Use this for:

- single-site apps
- compatibility defaults when explicit `[[sites]]` are not declared

When explicit `[[sites]]` are present, app-level domains become compatibility defaults rather than the primary site model.

## `[i18n]`

`[i18n]` defines app-wide locale policy.

Supported keys:

- `default_locale`
- `supported_locales`
- `localized_routes`

Use this for:

- single-site locale defaults
- app-wide locale bounds that all sites must stay within

When explicit `[[sites]]` are present:

- site-level locale policy is primary
- app-level locale settings act as compatibility defaults and validation bounds

## `[[sites]]`

`[[sites]]` is the first-class multi-site model.

Supported keys per site:

- `id`
- `display_name`
- `brand_name`
- `canonical_domain`
- `additional_domains`
- `default_locale`
- `supported_locales`
- `localized_routes`

Use sites when:

- one customer app serves multiple hostnames
- different markets need different default locales
- brand display differs by host
- route localization policy differs by site

Example:

```toml
[[sites]]
id = "shoppr-fr"
display_name = "Shoppr France"
brand_name = "Shoppr Paris"
canonical_domain = "fr.example.com"
additional_domains = ["www.fr.example.com"]
default_locale = "fr-FR"
supported_locales = ["en-GB", "fr-FR"]
localized_routes = true
```

Contract rules:

- each site must have one canonical domain
- site ids must be unique
- site domains must be unique across the app
- the site default locale must appear in the site's supported locales
- every site locale must also appear in the app's supported locales

## `[theme]`

`[theme]` tells the runtime which template and asset roots belong to the customer app.

Supported keys:

- `active`
- `template_namespaces`
- `asset_roots`

Typical meaning:

- `active`: logical theme id
- `template_namespaces`: ordered template lookup namespaces
- `asset_roots`: published customer asset roots

## `[auth]`

`[auth]` selects the auth package the app wants to run with.

Supported keys:

- `mode`
- `package`

Current mode values:

- `extend`
- `replace`

`app.toml` declares intent here. Runtime support for a given auth-package mode still depends on the current auth loader and validator.

## `[modules]`

`[modules]` declares which official modules the app installs.

Supported keys:

- `enabled`

Example:

```toml
[modules]
enabled = ["cms", "media", "commerce", "admin"]
```

This is the composition boundary for first-party batteries. Installing a module here means the app runtime must satisfy that module's capability and config contracts.

## `[[content_models]]`

Customer apps may declare customer-owned content model contracts in the manifest.

Supported keys per model:

- `id`
- `resource_kind`
- `fields`

Supported field keys:

- `id`
- `type`
- `localized`
- `required`

Supported field types:

- `text`
- `rich_text`
- `slug`
- `boolean`
- `integer`
- `date_time`
- `asset`
- `reference`

## `[[customer_migrations]]`

Customer migrations let the app declare app-owned migration contracts separate from core or module migrations.

Supported keys:

- `id`
- `order`
- `description`

## Common Mistakes

- Treating `app.toml` as a secret store. Database URLs, object store credentials, Stripe secrets, and similar runtime secrets do not belong here.
- Declaring both app-level and site-level locale/domain policy, then assuming both are equally authoritative. If `[[sites]]` are present, site policy is the real per-site source of truth.
- Installing modules in `[modules]` without supplying the matching auth capabilities or platform config they need.
- Using site-local locales that are not also listed in the app-wide supported locale set.
- Using `display_name` or `brand_name` as stable ids. They are presentation fields, not durable identifiers.

## Practical Rule

Keep `app.toml` product-shaped.

If a change is about:

- app identity
- hosts and sites
- locale policy
- theme/template roots
- auth package choice
- official module composition

it probably belongs here.

If it is about:

- secrets
- bind addresses
- TLS provider settings
- database/cache/storage backends
- observability

it belongs in platform config instead.
