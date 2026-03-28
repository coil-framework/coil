---
title: Non-Commerce Product Shape
---

Gitly is the clearest example of Davenda outside a retail lens. This guide walks through the
checked-in Gitly files that make the app feel like a forge rather than a storefront.

## Customer-Owned Routes, Not Store Routes

Gitly mounts its own route vocabulary in `apps/gitly/crates/gitly-app/src/lib.rs`.

The customer app owns routes such as:

- `/`
- `/explore`
- `/octocorp/platform-ui`
- `/octocorp/platform-ui/issues`
- `/octocorp/platform-ui/pulls`
- `/octocorp/platform-ui/actions`
- `/orgs/octocorp`
- `/alexmariner`
- `/search`

Those are not commerce routes with different styling. They are product-specific routes assembled by
the customer app itself.

The corresponding templates live under `apps/gitly/templates/gitly/`, where each page shows a
different type of information-dense application surface:

- `repository.html`
  - primary repository shell with API-hydrated summary data
- `issues.html`
  - issue-tracker style table
- `pulls.html`
  - review-focused pull-request table
- `actions.html`
  - scheduled workflow view
- `organization.html`
  - organization landing page
- `profile.html`
  - user profile page
- `search.html`
  - product-style search surface

That is the first Gitly lesson: a Davenda customer app can mount a completely different information
architecture while still using the same runtime and lifecycle patterns.

## Data Presentation Through Linked Rust

Gitly's public data is owned by the customer backend in
`apps/gitly/crates/gitly-backend/src/lib.rs`.

That file defines:

- `GitlyRepository`
- `GitlyPullRequest`
- `GitlyWorkflowRun`
- `GitlyOrganization`
- `GitlyUser`

It also exposes payload builders such as:

- `repo_api_payload()`
- `pulls_api_payload()`
- `workflow_api_payload()`
- `organization_api_payload()`
- `user_api_payload()`

Those helpers are then used by the customer app in
`apps/gitly/crates/gitly-app/src/lib.rs` when it mounts the custom API routes:

- `/api/github/repository`
- `/api/github/pulls`
- `/api/github/workflows`
- `/api/github/org`
- `/api/github/user`

This is a useful pattern to study because it shows how the customer app can keep presentation data
close to its own product shape instead of forcing everything through an ecommerce abstraction.

## Theme Switching And Product-Specific Frontend Behaviour

Gitly's frontend behaviour is intentionally customer-owned.

Read:

- `apps/gitly/theme/assets/site.css`
- `apps/gitly/theme/assets/site.js`

`site.css` provides the forge-like visual system:

- top bar and repository shell styling
- light and dark theme variables
- keyboard-visible focus states
- search, tabs, summary-card, and table styling

`site.js` provides the product behaviour:

- translation dictionaries for `en-GB`, `fr-FR`, and `de-DE`
- route-aware language switching
- `light`, `dark`, and `system` theme switching
- client-side API hydration for summary cards and counters
- the mock scheduler heartbeat on the Actions page
- search-form handling and localized labels

This is one of the most important Gitly lessons. The customer app can own a richer application
frontend without becoming a JavaScript-first shell.

## Localisation In A Non-Commerce Product

Gitly's localisation story is visible in three places:

- `apps/gitly/app.toml`
  - declares supported locales and localised routes
- `apps/gitly/platform.dev.toml`
  - repeats localized-route runtime config for the active environment
- `apps/gitly/theme/assets/site.js`
  - carries the translation tables and route-localisation behaviour

The templates under `apps/gitly/templates/gitly/` are written to support that runtime:

- they include `data-i18n-*` attributes for copy replacement
- they include `data-language-link` anchors for locale switching
- they include `data-route-link` hooks so the frontend can keep route-localized navigation coherent

This is not just a storefront language toggle transplanted into another app. It is a real
non-commerce example of localized product navigation and copy.

## Scheduled Tasks And API-Style Endpoints

Gitly is especially useful because it shows two different extension surfaces side by side.

In `apps/gitly/crates/gitly-app/src/lib.rs`, the customer module declares extension slots for:

- API: `/api/github/pulse`
- scheduled job: `github.actions.refresh`

The installed packages are pinned in `apps/gitly/app.toml` and described in:

- `apps/gitly/extensions/gitly-community-pulse/package.toml`
- `apps/gitly/extensions/gitly-actions-scheduler/package.toml`

The extension loader in `apps/gitly/crates/gitly-app/src/extensions.rs` compiles those checked-in
WAT sources into runtime artifacts during bootstrap.

That gives Gitly two useful product demonstrations:

- the community pulse card on `home.html` is backed by a bounded API extension surface
- the Actions page on `actions.html` can explain a customer-owned scheduled contract without
  pretending the app is a real CI system

The app also keeps the distinction visible in the UI:

- `apps/gitly/templates/gitly/home.html`
  calls out the WASM-backed community pulse surface
- `apps/gitly/templates/gitly/actions.html`
  explains the scheduled-job contract and shows the mock scheduler heartbeat panel

## CMS And Customer Hooks

Gitly is not a static site frozen in one product shell. Its linked backend also registers a CMS
hook through `apps/gitly/crates/gitly-backend/src/lib.rs`.

That hook matters because it proves Gitly still participates in the broader Davenda runtime model:

- the customer app can expose product-specific public routes
- it can still use CMS module surfaces
- linked Rust hooks can still enforce customer-specific policy during CMS publishing

So Gitly is not "a special GitHub clone mode." It is a normal Davenda customer app with a
different product shape.

## Suggested Reading Order

If you want to understand Gitly as a non-commerce product, read these files in order:

1. `apps/gitly/app.toml`
2. `apps/gitly/crates/gitly-app/src/lib.rs`
3. `apps/gitly/crates/gitly-backend/src/lib.rs`
4. `apps/gitly/templates/gitly/home.html`
5. `apps/gitly/templates/gitly/repository.html`
6. `apps/gitly/templates/gitly/actions.html`
7. `apps/gitly/theme/assets/site.js`
8. `apps/gitly/extensions/gitly-community-pulse/package.toml`
9. `apps/gitly/extensions/gitly-actions-scheduler/package.toml`

That sequence shows how Davenda's customer-app model survives intact when the product is not a
storefront at all.
