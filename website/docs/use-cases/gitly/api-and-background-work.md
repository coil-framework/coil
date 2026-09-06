---
title: API And Background Work
---

This page is about two reusable Coil patterns:

- customer-owned API-style endpoints
- bounded background work through explicit slots and jobs

Gitly is the example because it shows both patterns without commerce-specific noise.

## The Core Pattern

For a product that needs some JSON surfaces and some deferred work:

1. keep the main app server-rendered
2. mount customer-owned API routes in the app crate
3. use linked Rust for first-party payload shaping
4. use explicit extension slots or jobs for bounded add-on behaviour

That is the model Gitly demonstrates.

## Canonical API Route Pattern

Gitly’s app crate in `apps/gitly/crates/gitly-app/src/lib.rs` mounts product-shaped endpoints, and
the linked backend in `apps/gitly/crates/gitly-backend/src/lib.rs` supplies payload builders.

The useful thing to copy is the split, not the repo names:

```text
customer app crate
  -> defines routes and route ownership
linked backend crate
  -> builds typed product payloads for those routes
```

In Gitly’s case, the product surfaces include repository, pull, org, user, workflow, and pulse
data.

## Canonical Background-Work Slot Pattern

Gitly’s customer-owned showcase module declares a scheduled-job slot in
`apps/gitly/crates/gitly-app/src/lib.rs`:

```rust
ExtensionSlotDescriptor::new(
    ExtensionSlotKind::ScheduledJob,
    "github.actions.refresh",
    "Allows bounded third-party scheduled jobs to simulate GitHub Actions refresh cycles",
)
```

This snippet matters because it shows the platform pattern clearly:

- the customer app defines the product-specific job slot
- the runtime-installed extension plugs into that slot
- the platform still owns actual job execution and queueing

## Canonical API Extension Pattern

Gitly also declares an API slot in the same file:

```rust
ExtensionSlotDescriptor::new(
    ExtensionSlotKind::Api,
    "/api/github/pulse",
    "Allows bounded third-party extensions to contribute GitHub-style community pulse API data",
)
```

That is the cleanest non-commerce example of how to let runtime-installed packages contribute to an
API-shaped product surface without handing them ownership of the whole app.

## Gitly As The Supporting Example

### Linked backend payloads

Read:

- `apps/gitly/crates/gitly-backend/src/lib.rs`

This file provides the payload builders Gitly routes use for:

- repositories
- pulls
- workflows
- organizations
- users

### API extension package

Read:

- `apps/gitly/extensions/gitly-community-pulse/package.toml`
- `apps/gitly/crates/gitly-app/src/extensions.rs`

This pair shows:

- package metadata
- API target binding
- customer-app loader wiring

### Scheduled-job extension package

Read:

- `apps/gitly/extensions/gitly-actions-scheduler/package.toml`
- `apps/gitly/templates/gitly/actions.html`

This pair shows:

- scheduled-job target binding
- user-visible page that explains the background-work surface honestly

## Practical Rules To Copy

- keep API routes customer-owned and product-specific
- keep first-party payload shaping in linked Rust
- define explicit extension or job slots in the app layer
- keep the page shell server-rendered even if some surfaces hydrate from API endpoints

## Full Implementation Pointers

- `apps/gitly/crates/gitly-app/src/lib.rs`
- `apps/gitly/crates/gitly-app/src/extensions.rs`
- `apps/gitly/crates/gitly-backend/src/lib.rs`
- `apps/gitly/extensions/gitly-community-pulse/package.toml`
- `apps/gitly/extensions/gitly-actions-scheduler/package.toml`
- `apps/gitly/templates/gitly/actions.html`
- `apps/gitly/templates/gitly/home.html`

## Read Next

- [Extensions And Host APIs](./extensions-and-host-apis/)
- [Customer Rust Vs Third-Party WASM](../../reference/customer-vs-wasm/)
- [Ops Module Reference](../../reference/modules/ops/)
