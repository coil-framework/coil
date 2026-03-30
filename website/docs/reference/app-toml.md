---
title: app.toml
---

`app.toml` is the customer app manifest.

It describes the product-facing shape of an app:

- app identity
- domains and sites
- locale policy
- theme and template roots
- auth package selection
- installed official modules
- customer-owned content model and migration declarations
- runtime-installed extension installation entries

It is not the place for infrastructure secrets, runtime connection strings, or deployment-only tuning. Those belong in `platform.toml` or `platform.dev.toml`.

It is also not a request-time content payload. Declaring content models, sites, or extensions here
does not automatically populate CMS page instances, `page.blocks`, or customer-specific render-model
data.

## Why This File Exists

Coil separates product composition from runtime operations.

- `app.toml` answers "what kind of app is this?"
- `platform.toml` answers "how does this app run here?"

That split matters because a customer app should be able to keep its product contract stable while moving between development, staging, and production environments.

In practice, `app.toml` is the file a product developer edits when they need to:

- add a new site
- add a new locale
- enable or disable a module
- change the active theme roots
- switch to a different auth package
- install a runtime extension

If the change is about what the app is, it usually belongs here. If it is about how the app connects to infrastructure, it usually belongs in platform config instead.

If the change is about what a specific request should render right now, it usually does **not**
belong here.

## Shoppr As A Concrete Example

The checked-in Shoppr manifest is the best current reference example:

- `apps/shoppr/app.toml`

That file demonstrates:

- a three-site market layout
- shared app-wide locale defaults
- theme roots
- an extending auth package
- the full official module battery
- a runtime-installed WASM extension entry

When reading the field descriptions below, keep the Shoppr manifest open beside this page. That is the current canonical working example.

## Supported Top-Level Sections

The current manifest loader supports these top-level sections:

- `[app]`
- `[domains]`
- `[i18n]`
- `[[sites]]`
- `[theme]`
- `[auth]`
- `[modules]`
- `[[extensions]]`
- `[[content_models]]`
- `[[customer_migrations]]`

Not every application needs every section. The most common baseline is:

- `[app]`
- `[domains]`
- `[i18n]`
- `[theme]`
- `[auth]`
- `[modules]`

Multi-site apps add `[[sites]]`. Apps with runtime-installed extensions add `[[extensions]]`. Apps that carry custom content contracts or migration runbook entries add `[[content_models]]` and `[[customer_migrations]]`.

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

This is enough to express a single-site product shape. It does not yet describe runtime concerns such as the database URL, storage backend, or TLS mode.

It also does not create page records, block instances, or template-facing request data by itself.

## Multi-Site Example

The moment you need per-market hostnames, branding, or locale defaults, `[[sites]]` becomes the primary model:

```toml
[app]
name = "shoppr"
display_name = "Shoppr"

[domains]
canonical = "uk.example.com"
additional = ["fr.example.com", "pl.example.com"]

[i18n]
default_locale = "en-GB"
supported_locales = ["en-GB", "fr-FR", "pl-PL"]
localized_routes = true

[[sites]]
id = "shoppr-uk"
display_name = "Shoppr UK"
brand_name = "Shoppr"
canonical_domain = "uk.example.com"
additional_domains = ["www.example.com"]
default_locale = "en-GB"
supported_locales = ["en-GB", "fr-FR", "pl-PL"]

[[sites]]
id = "shoppr-fr"
display_name = "Shoppr France"
brand_name = "Shoppr Paris"
canonical_domain = "fr.example.com"
default_locale = "fr-FR"
supported_locales = ["en-GB", "fr-FR", "pl-PL"]
```

If you are unsure whether you need `[[sites]]` or just more locales, read [Sites, locales, and markets](../core-concepts/sites-locales-and-markets.md).

## Extension Installation Example

Runtime-installed WASM extensions are declared in `app.toml`, not in platform config:

```toml
[[extensions]]
id = "shoppr-waitlist-tools"
package_version = "0.1.0"
artifact_sha256 = "3ad7b44218d04a3eba602051cbcb991bdd1ab69fd55ad995cd688af26ca6d067"
customer_app_id = "shoppr"

[[extensions.handlers]]
id = "home.waitlist.banner"
grants = []
```

Shoppr’s real installation entry is here:

- `apps/shoppr/app.toml`

The extension package itself lives here:

- `apps/shoppr/extensions/shoppr-waitlist-tools/package.toml`

If you need the full extension lifecycle, packaging, or handler model, read [Customer Rust vs third-party WASM](customer-vs-wasm.md).

The presence of an extension entry here does not mean that extension automatically shapes every page
model. Execution still depends on the runtime integration path and the specific handler or host API
surface involved.

## `[app]`

`[app]` defines the customer app identity.

Supported keys:

- `name`
- `display_name`

### What These Keys Mean

- `name`: the stable app id used across composition and runtime contracts
- `display_name`: the human-readable product name shown in docs, admin surfaces, and descriptive output

### Example

```toml
[app]
name = "shoppr"
display_name = "Shoppr"
```

### Guidance

- Treat `name` as durable. It should not change casually once the app exists in real environments.
- Treat `display_name` as presentation. It can evolve without changing the app’s identity.
- If omitted, `display_name` falls back to `name`.

## `[domains]`

`[domains]` is the app-level compatibility domain block.

Supported keys:

- `canonical`
- `additional`

### What This Section Means

This block gives the app a global default hostname view. It is sufficient for:

- single-site applications
- simple applications with a canonical hostname and aliases
- compatibility defaults in apps that also declare `[[sites]]`

### Example

```toml
[domains]
canonical = "shop.example.com"
additional = ["www.example.com"]
```

### Guidance

Use this section by itself if the app is genuinely single-site.

When explicit `[[sites]]` are present, app-level domains become compatibility defaults rather than the primary site model. In other words:

- `[[sites]]` is the real source of truth for per-site hosts
- `[domains]` remains useful as a top-level summary and compatibility layer

This is still site composition, not request-time page data.

## `[i18n]`

`[i18n]` defines app-wide locale policy.

Supported keys:

- `default_locale`
- `supported_locales`
- `localized_routes`

### What This Section Means

This block defines the outer language bounds for the application.

It is used for:

- single-site locale defaults
- app-wide validation bounds for multi-site locale policy
- whether routes are locale-prefixed

### Example

```toml
[i18n]
default_locale = "en-GB"
supported_locales = ["en-GB", "fr-FR", "pl-PL"]
localized_routes = true
```

### Guidance

- `default_locale` must appear in `supported_locales`
- if `[[sites]]` are present, each site’s locales must stay within the app-wide supported set
- `localized_routes = true` is the normal choice for public multi-lingual apps

For translation files, fallback chains, and template syntax, read [Internationalisation](internationalization.md).

This section sets locale policy. It does not translate templates or content automatically.

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

### What This Section Means

Use sites when one customer app needs multiple market or brand surfaces that share the same binary and broad product family but differ in operational or product-facing ways such as:

- hostnames
- default locale
- supported locales
- brand display
- assortment, pricing, or content differences through downstream config and content

### Example

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

### Contract Rules

- each site must have one canonical domain
- site ids must be unique
- site domains must be unique across the app
- the site default locale must appear in the site’s supported locales
- every site locale must also appear in the app’s supported locales

### Guidance

If the only difference between two audiences is language, add a locale. If inventory, fulfilment, launch calendars, pricing, branding, or operational policy diverge, add a site instead.

## `[theme]`

`[theme]` tells the runtime which template and asset roots belong to the customer app.

Supported keys:

- `active`
- `template_namespaces`
- `asset_roots`

### Meaning

- `active`: logical theme id
- `template_namespaces`: ordered template lookup namespaces
- `asset_roots`: asset directories that should be published as customer theme assets

### Example

```toml
[theme]
active = "harbor"
template_namespaces = ["customer-app", "harbor"]
asset_roots = ["theme/assets"]
```

### Guidance

- Keep `template_namespaces` in explicit lookup order
- `asset_roots` should only list roots that you want Coil to publish
- theme structure and template lookup are documented in [Theme structure](theme-structure.md)

## `[auth]`

`[auth]` selects the auth package the app wants to run with.

Supported keys:

- `mode`
- `package`

Current mode values:

- `extend`
- `replace`

### Example

```toml
[auth]
mode = "extend"
package = "shoppr-auth"
```

### Guidance

`app.toml` declares intent here. Runtime support for a given auth-package mode still depends on the current auth loader and validator, but on current `main` both extending and replacement file-backed package paths are supported.

Use:

- `extend` when the default platform auth vocabulary is mostly right and you need to refine it
- `replace` when your domain needs a fully customer-owned authorisation model

Read next:

- [Auth overview](auth-overview.md)
- [Auth packages](auth-packages.md)
- [Custom auth schema guidance](custom-auth-schema.md)

## `[modules]`

`[modules]` declares which official modules the app installs.

Supported keys:

- `enabled`

### Example

```toml
[modules]
enabled = ["cms", "media", "commerce", "admin"]
```

### Guidance

This is the composition boundary for first-party batteries. Installing a module here means the app runtime must satisfy that module’s capability and config contracts.

Common patterns:

- start with `coil` plus a broad enabled set while evaluating Coil
- narrow the dependency graph later if you need tighter composition control
- keep `[modules]` aligned with what the customer binary actually links

For module-specific behaviour, use the dedicated module pages under [Official modules](modules.md).

## `[[extensions]]`

Customer apps may declare runtime-installed WASM extensions directly in the manifest.

Supported keys:

- `id`
- `package_version`
- `artifact_sha256`
- `customer_app_id`

Supported handler keys:

- `id`
- `grants`

### What This Section Means

This is the installation contract between a customer app and a runtime-only extension artifact. The manifest says:

- which extension package is being installed
- which compiled artifact checksum is expected
- which handler ids are enabled for this app

### Guidance

- keep the declared checksum aligned with the compiled artifact
- treat the installation entry as product composition, not infrastructure
- use linked Rust for customer-owned first-party logic and WASM for runtime-installed extension logic

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

### What This Section Means

This is where a customer app declares additional structured content it owns beyond the official module defaults.

That distinction matters:

- `[[content_models]]` is schema and ownership
- CMS records are instances and stored content
- request-time model shaping decides what templates actually receive

### Narrative Guidance

Do not think of `[[content_models]]` as just a field list. Think of it as the public contract for customer-managed structured content. The field list matters because it drives:

- validation
- editing surfaces
- localisation behaviour
- routing or lookup patterns when slugs are involved

If you add a model such as `lookbook_entry` or `campaign_page`, document its intended editor workflow as well as its field list.

If this boundary is fuzzy, read
[Content schema vs content instances](../core-concepts/content-schema-vs-content-instances.md)
and [CMS page builder model](./cms-page-builder-model.md).

## `[[customer_migrations]]`

Customer migrations let the app declare app-owned migration contracts separate from core or module migrations.

Supported keys:

- `id`
- `order`
- `description`

### What This Section Means

This block records migration work owned by the customer application rather than the platform core or an official module.

### Example

```toml
[[customer_migrations]]
id = "shoppr-loyalty-rollout"
order = 500
description = "Create loyalty segmentation tables for linked customer backend rules"
```

### Guidance

- use durable ids
- keep ordering explicit
- describe the operational purpose of the migration, not just the technical action

The runtime plan and operator tooling can then surface these as customer-owned migration obligations rather than pretending everything is core-owned.

## Common Mistakes

- Treating `app.toml` as a secret store. Database URLs, object store credentials, Stripe secrets, and similar runtime secrets do not belong here.
- Declaring both app-level and site-level locale/domain policy, then assuming both are equally authoritative. If `[[sites]]` are present, site policy is the real per-site source of truth.
- Installing modules in `[modules]` without supplying the matching auth capabilities or platform config they need.
- Using site-local locales that are not also listed in the app-wide supported locale set.
- Using `display_name` or `brand_name` as stable ids. They are presentation fields, not durable identifiers.
- Letting extension checksums drift from the built artifact.
- Treating `app.toml` as live page content. It does not create page instances or populate `page.blocks`.
- Treating content schema as a full render contract. Request-time render data still has to be shaped explicitly by framework code, official modules, or customer hooks.

## Practical Rule

If a change alters the product contract visible to developers, editors, operators, or the runtime planner, it probably belongs in `app.toml`.

If the change is about environment wiring, secrets, scaling, or infrastructure, it probably belongs in `platform.toml`.

## Read Next

- [platform.toml and platform.dev.toml](platform-config.md)
- [Sites, locales, and markets](../core-concepts/sites-locales-and-markets.md)
- [Content schema vs content instances](../core-concepts/content-schema-vs-content-instances.md)
- [Render pipeline and model composition](../core-concepts/render-pipeline-and-model-composition.md)
- [CMS page builder model](./cms-page-builder-model.md)
- [Getting Started: Add a Real Content Model](../getting-started/add-a-real-content-model.md)
- [Theme structure](theme-structure.md)
- [Official modules](modules.md)
