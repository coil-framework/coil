---
title: Gitly Extensions And Host APIs
---

This page is about the platform extension model first, with Gitly as the supporting example.

Use it when you want to answer:

- how a customer app should expose extension slots
- how runtime-installed packages bind to those slots
- how to explain host APIs without making one demo app the whole story

## The Core Pattern

For runtime-installed extensions, keep the responsibilities split:

1. the customer app defines named extension slots in product vocabulary
2. the customer app installs packages explicitly in `app.toml`
3. the package manifest binds handlers to those targets
4. the runtime enforces grants and host-API boundaries

Gitly demonstrates this especially well because it uses one API slot and one scheduled-job slot.

## Canonical Extension Slot Pattern

Gitly’s customer app module in `apps/gitly/crates/gitly-app/src/lib.rs` defines extension slots
like this:

```rust
ExtensionSlotDescriptor::new(
    ExtensionSlotKind::Api,
    "/api/github/pulse",
    "Allows bounded third-party extensions to contribute GitHub-style community pulse API data",
)
```

and:

```rust
ExtensionSlotDescriptor::new(
    ExtensionSlotKind::ScheduledJob,
    "github.actions.refresh",
    "Allows bounded third-party scheduled jobs to simulate GitHub Actions refresh cycles",
)
```

These snippets are the most important part of the page because they show the real pattern:

- the customer app owns the product slot names
- extensions plug into explicit, app-defined targets

## Canonical Install Pattern

The customer app then installs packages in `app.toml`.

Gitly’s `apps/gitly/app.toml` uses:

```toml
[[extensions]]
id = "gitly-community-pulse"
package_version = "0.1.0"
artifact_sha256 = "..."
customer_app_id = "gitly"
```

That is the right installation boundary:

- explicit package id
- explicit version
- explicit artifact hash
- explicit customer ownership

## Canonical Package Binding Pattern

The package itself binds handlers to extension points in `package.toml`.

Gitly’s API package example:

```toml
[[handlers]]
id = "community-pulse"
export = "exports.community_pulse"
point = "api"
target = "/api/github/pulse"
grants = []
```

Gitly’s scheduled-job example:

```toml
[[handlers]]
id = "nightly-refresh"
export = "exports.nightly_refresh"
point = "scheduled-job"
target = "github.actions.refresh"
grants = []
```

Those snippets show the complete binding chain:

- app defines slot
- app installs package
- package binds handler to slot target

## Gitly As The Supporting Example

### App-defined slots

Read:

- `apps/gitly/crates/gitly-app/src/lib.rs`

This is where Gitly defines the product’s extension vocabulary.

### Package loading

Read:

- `apps/gitly/crates/gitly-app/src/extensions.rs`

This file shows the customer-app loader path:

1. read installed extensions from `app.toml`
2. load `extensions/<id>/package.toml`
3. compile demo artifacts
4. build `ExtensionPackage`
5. attach installation data and grants

### Installed packages

Read:

- `apps/gitly/extensions/gitly-community-pulse/package.toml`
- `apps/gitly/extensions/gitly-actions-scheduler/package.toml`

### Product surfaces

Read:

- `apps/gitly/templates/gitly/home.html`
- `apps/gitly/templates/gitly/actions.html`
- `apps/gitly/theme/assets/site.js`

These are the app-visible surfaces that make the extension contributions legible.

## Host API Boundary

The general host API and grant model is documented in:

- [WASM Host APIs](../../reference/wasm-host-apis/)
- [Extension Package Format](../../reference/extension-package-format/)

Gitly’s handlers use empty grant sets intentionally. That keeps the demo focused on slot design and
package binding instead of grant complexity.

## Practical Rules To Copy

- define extension slots in the customer app before shipping packages
- make slot names match real product vocabulary
- install packages explicitly in `app.toml`
- keep package targets and slot targets aligned exactly

## Full Implementation Pointers

- `apps/gitly/crates/gitly-app/src/lib.rs`
- `apps/gitly/crates/gitly-app/src/extensions.rs`
- `apps/gitly/app.toml`
- `apps/gitly/extensions/gitly-community-pulse/package.toml`
- `apps/gitly/extensions/gitly-actions-scheduler/package.toml`
- `apps/gitly/templates/gitly/home.html`
- `apps/gitly/templates/gitly/actions.html`
- `apps/gitly/theme/assets/site.js`

## Read Next

- [API And Background Work](./api-and-background-work/)
- [Build And Deploy](./build-and-deploy/)
- [Extension Package Format](../../reference/extension-package-format/)
