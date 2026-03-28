---
title: Product Structure
---

Gitly demonstrates how to build a non-commerce product shell in Davenda without fighting the
platform.

## The Files That Define The Product Shape

Start with:

- `apps/gitly/crates/gitly-app/src/lib.rs`
- `apps/gitly/templates/gitly/home.html`
- `apps/gitly/templates/gitly/explore.html`
- `apps/gitly/templates/gitly/repository.html`
- `apps/gitly/templates/gitly/issues.html`
- `apps/gitly/templates/gitly/pulls.html`
- `apps/gitly/templates/gitly/actions.html`
- `apps/gitly/templates/gitly/organization.html`
- `apps/gitly/templates/gitly/profile.html`
- `apps/gitly/templates/gitly/search.html`

That list shows the most important Gitly idea: the customer app owns the product vocabulary.

## Customer-Owned Routes, Not Store Routes

Gitly mounts routes such as:

- `/`
- `/explore`
- repository pages
- issues pages
- pull request pages
- actions pages
- organization pages
- profile pages
- search pages

Those routes are assembled by the customer app in `apps/gitly/crates/gitly-app/src/lib.rs`.

This is the cleanest repo example of Davenda hosting a product that is not shaped like a storefront.

## What Each Template Teaches

- `home.html`
  - landing page plus API-driven summary surfaces
- `repository.html`
  - dense product shell for repository data
- `issues.html`
  - issue-tracker style listing
- `pulls.html`
  - review-centric table and summary layout
- `actions.html`
  - scheduled-task and workflow demo surface
- `organization.html`
  - organization landing page
- `profile.html`
  - user identity and activity surface
- `search.html`
  - application-style search experience

Each template shows that Davenda's HTML-first model still works for non-commerce UIs.

## Where The Data Comes From

Gitly's linked backend in `apps/gitly/crates/gitly-backend/src/lib.rs` provides:

- repository fixtures
- pull request fixtures
- workflow fixtures
- organization fixtures
- user fixtures
- API payload builders

That gives the customer app a product-shaped data source without forcing it through commerce or CMS
abstractions.

## Adapt This For Your Own Product

If you are building a non-commerce app, copy these ideas from Gitly:

- define product-specific routes in the customer app
- let templates match the real product vocabulary
- use linked Rust for product-shaped data and policy
- use official modules only where they genuinely help

## Read Next

- [Theming, Localization, And Accessibility](./theming-localization-and-accessibility.md)
- [API And Background Work](./api-and-background-work.md)
- [Gitly Overview](./overview.md)
