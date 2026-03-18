# Upgrades, Compatibility, and Release Management

**Part:** Customer Apps  
**Chapter:** 81

Customer apps are expected to evolve continuously, but they only remain upgradeable if they consume the platform through declared contracts. In this platform the critical distinction is between core, official modules, and the customer app itself. Core owns the runtime, auth engine, template engine, cache, storage, TLS, i18n, SEO primitives, and the WASM host. Official modules provide installable business capability such as CMS, catalog, checkout, memberships, events, and admin shells. The customer app owns frontend composition, templates, translations, configuration, content model, and customer-specific extensions. Release management exists to preserve those boundaries over time.

## Compatibility Model

Compatibility is defined at the contract level, not at the implementation-detail level. Customer apps are expected to bind to:

- stable HTTP, rendering, storage, cache, auth, and extension APIs exposed by core
- capability contracts exposed by official modules
- published configuration schema and CLI behavior
- versioned auth model packages and capability bindings

Customer apps are not expected to depend on internal database tables, undeclared module events, or direct access to auth tuple storage. A customer app that reaches behind those contracts may still work temporarily, but it is no longer covered by the platform’s compatibility policy.

The same rule applies to extensions. WASM packages target host APIs and capability contracts, not native crate internals. That keeps upgrades tractable and prevents a WordPress-style ecosystem of hidden coupling.

## What Gets Versioned

A customer deployment is a composition of independently versioned artifacts:

- core runtime
- official native modules
- customer app package
- auth model package
- capability registry
- WASM extension packages
- configuration schema revisions and storage-policy revisions where those are published as versioned documents

These pieces move together operationally, but they are not one version number. Core can ship a patch release without forcing a catalog-module release. A customer app can ship a frontend or content-model change without changing the auth engine. The release process therefore publishes a compatibility matrix as well as individual package versions.

## Upgrade Workflow

Every upgrade should be run as a composition exercise instead of a blind package bump.

1. Select the target versions for core, official modules, auth model package, and app package from the published compatibility matrix.
2. Generate an upgrade plan that lists config changes, schema migrations, auth-model migrations, extension ABI changes, and cache or storage policy changes.
3. Apply the plan in a staging environment using production-like data and object-store access patterns.
4. Replay the critical user journeys for that customer app: public page rendering, localized routing, login, account management, checkout or booking flow, admin editing, asset publication, and background jobs.
5. Roll out gradually in production, starting with a canary or a small traffic slice.

The upgrade toolchain should treat “no compatible path” as a hard failure. If a target module version requires a newer core or a newer capability binding set, the tool should refuse to proceed until the version graph is coherent.

## Schema and Data Changes

Schema changes are owned by the layer that defines the data.

- Core migrations own host-runtime tables such as auth tuple storage, queue state, cache metadata, or system-level configuration tables.
- Official module migrations own their module data, such as pages, products, orders, memberships, events, bookings, or media metadata.
- Customer apps may own app-local tables for custom content or integration state, but they must not patch official-module tables in place.

The same separation applies to auth. The tuple storage schema, the authorization model, and the capability bindings are distinct versioned concerns. A migration may update one without changing the others, and release notes must say which one moved.

## Release Types

Three release types are enough for day-to-day governance:

- Patch releases fix defects or security issues without changing published contracts.
- Minor releases add optional capabilities, new module features, new host APIs, or new configuration fields while preserving backward compatibility within the current major line.
- Major releases are reserved for contract changes such as removed host APIs, breaking config changes, auth-model package format changes, or incompatible module behavior.

Within a major line, deprecated surfaces remain supported until the next major release. The deprecation must be documented in release notes, surfaced by the CLI where possible, and detectable in staging before production rollout.

## Operational Release Discipline

Release management is not complete when artifacts build successfully. Each release set should carry:

- a compatibility matrix for core, modules, auth model packages, and WASM host ABI versions
- migration notes for schema, storage policy, cache behavior, and TLS or CDN implications
- rollback instructions that identify which parts are stateless to reverse and which parts require data reconciliation
- test evidence for the reference commerce and events-plus-memberships installations

For customer apps that use progressive enhancement, release validation must include both full-page responses and fragment endpoints. Cache scoping also needs explicit review because a change in auth-aware caching can leak personalized content if misconfigured.

## Recommended Customer-App Practice

Customer apps should pin exact artifact versions in deployment manifests, but follow the platform’s supported versions closely. Long-lived divergence is expensive because official modules keep moving, capability registries evolve, and security fixes in core are not optional. The goal is not permanent customization branches. The goal is a stable customer app that can keep consuming the platform as it improves.
