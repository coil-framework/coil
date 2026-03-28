---
title: Gitly Overview
---

Gitly is the main non-commerce teaching app in the repo. It proves that Davenda can power a
developer-product shell, not just a store.

Gitly is useful because it keeps the same customer-app model as Shoppr while changing the product
shape completely.

## What Gitly Is Showing

Gitly demonstrates all of these in one app:

- customer-owned routes and templates
- linked Rust data shaping
- API-style endpoints
- theme switching
- localized UI copy
- scheduled-task demos
- runtime-installed WASM
- customer-owned lifecycle commands

## Start With The App Contract

Read these files first:

1. `apps/gitly/app.toml`
2. `apps/gitly/platform.dev.toml`
3. `apps/gitly/crates/gitly-app/src/lib.rs`
4. `apps/gitly/crates/gitly-bin/src/main.rs`

That sequence tells you:

- what Gitly claims to be
- how the runtime is configured
- how the customer composition root builds the product shell
- how the customer binary owns validate, assets, migrate, serve, and up

## What Gitly Enables

Gitly's enabled module set is narrow on purpose:

- `admin`
- `cms`
- `media`
- `gitly-showcase`

That is one of the best lessons in the repo. Davenda does not need commerce to be coherent.

Gitly builds a non-commerce product by:

- using a small official module set
- adding customer-owned routes and templates
- adding a customer-owned showcase module with extension slots

## How The Workspace Is Structured

Important folders:

- `apps/gitly/crates/gitly-app`
  - composition root and route registration
- `apps/gitly/crates/gitly-backend`
  - linked customer backend and API payload builders
- `apps/gitly/crates/gitly-bin`
  - customer binary lifecycle commands
- `apps/gitly/extensions`
  - runtime-installed API and scheduled-job packages
- `apps/gitly/templates/gitly`
  - product pages
- `apps/gitly/theme`
  - CSS, JS, and tokens

That is the same customer-root shape as Shoppr, but the product vocabulary is completely
different.

## The Product Surface

The public Gitly templates live under `apps/gitly/templates/gitly/`:

- `home.html`
- `explore.html`
- `repository.html`
- `issues.html`
- `pulls.html`
- `actions.html`
- `organization.html`
- `profile.html`
- `search.html`

These are the files to read when you want to see Davenda from a non-commerce lens.

## Linked Rust And WASM In Gitly

Gitly also demonstrates both extension models clearly.

Linked Rust:

- `apps/gitly/crates/gitly-backend/src/lib.rs`

WASM:

- `apps/gitly/extensions/gitly-community-pulse/package.toml`
- `apps/gitly/extensions/gitly-actions-scheduler/package.toml`
- `apps/gitly/crates/gitly-app/src/extensions.rs`

This matters because Gitly shows the same platform extension story without any commerce framing.

## Why Gitly Matters

Use Gitly when you want to show a skeptical teammate that Davenda is not “only for stores.”

Gitly demonstrates:

- custom route vocabulary
- app-style data presentation
- localized product UI
- a theme switcher
- bounded background-work demos
- API-style extension points

without leaving the customer-root model.

## Read Next

- [Product Structure](./product-structure.md)
- [Theming, Localization, And Accessibility](./theming-localization-and-accessibility.md)
- [API And Background Work](./api-and-background-work.md)
- [Non-Commerce Product Shape](./non-commerce-product-shape.md)
