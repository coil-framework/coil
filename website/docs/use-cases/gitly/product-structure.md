---
title: Product Structure
---

This page is about how to structure a non-commerce Davenda product, with Gitly as the example.

Use it when you want to answer:

- where customer-owned routes belong
- how templates map to product vocabulary
- where linked Rust should supply non-commerce data

## The Core Pattern

For a non-commerce app, keep the product structure split across:

1. a customer app crate that defines the route vocabulary
2. templates that match the real product nouns
3. linked Rust that shapes product data
4. only the modules that genuinely help

Gitly is useful because it keeps that split very visible.

## Canonical Route Vocabulary Pattern

A customer app should own the product’s route nouns directly. Gitly’s composition root in
`apps/gitly/crates/gitly-app/src/lib.rs` is the concrete example.

The product routes it mounts are shaped around repository software, not storefront pages:

```text
/
/explore
/:owner/:repo
/:owner/:repo/issues
/:owner/:repo/pulls
/:owner/:repo/actions
/orgs/:org
/:user
/search
```

That is the key lesson:

- the platform should not force commerce-shaped routes
- the customer app should define the product vocabulary directly

The important concrete detail is that Gitly also maps those routes to templates in the same app
crate:

```rust
for (route, template) in gitly_page_routes() {
    let route_name = route.name.clone();
    ensure_route(runtime, route)?;
    ensure_handler(runtime, HandlerDefinition::page(route_name, template)?)?;
}
```

With entries such as:

```rust
("repo", "/forgeflow/platform-ui", "gitly/repository")
```

That is the real template tie-in. The template is not discovered by filename convention alone. The
customer app chooses it.

## Canonical Template Mapping Pattern

Once the route vocabulary exists, templates should mirror that vocabulary one-for-one.

Gitly’s template tree under `apps/gitly/templates/gitly/` does exactly that:

- `home.html`
- `explore.html`
- `repository.html`
- `issues.html`
- `pulls.html`
- `actions.html`
- `organization.html`
- `profile.html`
- `search.html`

This is the pattern to copy for any Davenda app:

- name templates after real product surfaces
- keep them product-first, not framework-first

## Canonical Linked-Data Pattern

A non-commerce app still needs product-shaped data. Gitly uses linked Rust for that in
`apps/gitly/crates/gitly-backend/src/lib.rs`.

The backend supplies:

- repository fixtures
- pull request fixtures
- workflow fixtures
- organization fixtures
- user fixtures
- API payload builders

That is the right boundary for product-shaped data that belongs to one customer app but is too rich
for static templates alone.

## What Gitly Demonstrates Today, And What It Does Not

Gitly is a good example of:

- customer-owned route vocabulary
- customer-owned route-to-template mapping
- linked Rust for domain fixtures and policy
- custom JSON endpoints and runtime-installed WASM surfaces

Gitly is **not** yet the strongest example of a customer-owned server-side `RenderModel` builder
for those custom routes.

Today its repository, profile, and actions pages lean more heavily on:

- static HTML structure
- `data-*` attributes
- client-side localisation and enhancement
- separate custom JSON endpoints for GitHub-style API payloads

So if you want to understand "how does a route get tied to a template?" Gitly is a strong example.
If you want to understand "where does the server-side page model get shaped?" Shoppr is currently
the stronger example.

## Gitly As The Supporting Example

### Customer-owned routes

Full implementation:

- `apps/gitly/crates/gitly-app/src/lib.rs`
- `apps/gitly/templates/gitly/home.html`
- `apps/gitly/templates/gitly/repository.html`
- `apps/gitly/templates/gitly/actions.html`
- `apps/gitly/crates/gitly-backend/src/lib.rs`
- `apps/gitly/theme/assets/site.js`

Use those after reading this page, not instead of reading this page.

## Practical Rules To Copy

- define your route vocabulary in the customer app crate
- keep template names aligned to product nouns
- let linked Rust provide product-shaped data and policy
- only enable modules that support the product instead of forcing a broad stack

## Full Implementation Pointers

- `apps/gitly/crates/gitly-app/src/lib.rs`
- `apps/gitly/crates/gitly-backend/src/lib.rs`
- `apps/gitly/templates/gitly/home.html`
- `apps/gitly/templates/gitly/explore.html`
- `apps/gitly/templates/gitly/repository.html`
- `apps/gitly/templates/gitly/issues.html`
- `apps/gitly/templates/gitly/pulls.html`
- `apps/gitly/templates/gitly/actions.html`
- `apps/gitly/templates/gitly/organization.html`
- `apps/gitly/templates/gitly/profile.html`
- `apps/gitly/templates/gitly/search.html`

## Read Next

- [Theming, Localisation, And Accessibility](./theming-localization-and-accessibility.md)
- [API And Background Work](./api-and-background-work.md)
- [Gitly Overview](./overview.md)
