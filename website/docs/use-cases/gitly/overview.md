---
title: Gitly Overview
---

Gitly is the main non-commerce teaching app in the repo. It proves that the same platform used for
Shoppr can also power a GitHub-like product shell with repository pages, mock APIs, localization,
theme switching, scheduled-task demos, and customer-owned extension points.

## Why Gitly Matters

Gitly exists to answer a simple question:

Can Davenda still look coherent when the product is not a store?

The answer in the checked-in app is yes, because the same customer-app boundary still owns:

- the app manifest
- runtime config
- templates
- theme assets
- linked Rust backend code
- runtime-installed WASM packages
- customer binary lifecycle commands

## Start With The App Shape

Read these files first:

1. `apps/gitly/app.toml`
   - declares the app id, domains, locales, active theme, modules, and installed WASM packages
   - pins the API extension and scheduled-job extension in the same way Shoppr pins its extension
2. `apps/gitly/platform.dev.toml`
   - supplies the local runtime config, including Redis, Postgres, object storage, jobs, SEO, and
     localized routes
3. `apps/gitly/crates/gitly-app/src/lib.rs`
   - is the customer composition root
   - defines the `gitly-showcase` module and its extension slots
   - loads the linked Rust backend and runtime-installed extensions
   - mounts GitHub-style routes and JSON endpoints
4. `apps/gitly/crates/gitly-bin/src/main.rs`
   - turns the app into a customer-owned binary with `describe`, `validate`, `assets`, `migrate`,
     `serve`, `up`, and linked-backend inspection commands

That four-file path is the clearest demonstration that Gitly is a real customer app, not a set of
loose demo pages.

## The Product Surface

Gitly's public surface lives in `apps/gitly/templates/gitly/`:

- `home.html`
  - the landing page for the forge-style demo
  - includes API-hydrated summary cards and the community pulse widget
- `repository.html`
  - shows the main repository shell
- `issues.html`
  - keeps the issue-tracker shape honest and static
- `pulls.html`
  - shows pull-request review data in a product-specific layout
- `actions.html`
  - demonstrates the scheduled-job surface and mock scheduler heartbeat
- `organization.html`
  - shows a non-commerce organization page
- `profile.html`
  - shows a user profile page
- `search.html`
  - demonstrates an application-style search page rather than a storefront page

The important lesson is that Davenda does not force the customer app into commerce-shaped routes.
Gitly mounts a completely different route vocabulary and still uses the same customer-root model.

## Theme Switching And Localization

Gitly's theme and frontend interaction layer are customer-owned:

- `apps/gitly/theme/assets/site.css`
  - defines the GitHub-like visual system
  - uses `html[data-theme="dark"]` for dark-mode theming
  - includes the focus and navigation styling that make the shell work as a product UI
- `apps/gitly/theme/assets/site.js`
  - owns the theme switcher
  - owns the locale switching and translated copy tables
  - hydrates API summary fields client-side
  - simulates the scheduled-job heartbeat on the Actions page

This is one of the most useful Gitly lessons: a customer app can own a strongly product-specific
frontend personality without becoming a single-page application or abandoning server-rendered
templates.

## Linked Rust Backend And WASM

Gitly also shows both extension paths clearly.

Linked Rust:

- `apps/gitly/crates/gitly-backend/src/lib.rs`
  - defines repository, pull request, workflow, organization, and user data fixtures
  - exposes GitHub-style API payload builders
  - registers a CMS publish hook
  - publishes a linked-plugin summary used by the CLI

Runtime-installed WASM:

- `apps/gitly/extensions/README.md`
  - explains why Gitly uses WASM only for bounded runtime-installed behavior
- `apps/gitly/extensions/gitly-community-pulse/package.toml`
  - declares the API extension for `/api/github/pulse`
- `apps/gitly/extensions/gitly-actions-scheduler/package.toml`
  - declares the scheduled-job extension for `github.actions.refresh`
- `apps/gitly/crates/gitly-app/src/extensions.rs`
  - loads and compiles those packages during bootstrap

That split is the core architectural lesson:

- linked Rust for first-party customer logic
- WASM for narrower runtime-installed behavior

## What To Read Next

Use the companion Gitly guide for the detailed product walkthrough:

- `non-commerce-product-shape`
  - route structure
  - API-style data presentation
  - theme switching and localization
  - scheduled tasks
  - linked backend and WASM extension hooks
