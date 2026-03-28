# Davenda

[![CI](https://github.com/davenda/davenda/actions/workflows/ci.yml/badge.svg)](https://github.com/davenda/davenda/actions/workflows/ci.yml)
[![Docs](https://github.com/davenda/davenda/actions/workflows/docs.yml/badge.svg)](https://github.com/davenda/davenda/actions/workflows/docs.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-black.svg)](./LICENSE)

Davenda is a highly opinionated Rust web framework for teams that want to ship real products, not assemble a web stack from spare parts.

It leads with ecommerce because that is where the architecture is most demanding:

- content and merchandising need to move fast
- checkout, payments, auth, and customer data need to be safe
- operations need to stay boring under load
- product teams still need room for customer-specific behaviour

But Davenda is not ecommerce-only. The same platform shape also powers non-commerce products like the checked-in Gitly demo. The point is not "Rust for storefronts only." The point is "Rust for serious web applications, with batteries included and strong opinions about where complexity belongs."

## Why Developers Reach For Davenda

- HTML-first by default, with progressive enhancement layered on instead of forcing a SPA-first architecture.
- Multi-site, multi-locale, and market-aware routing built into the product model instead of bolted on later.
- First-party Rust extension model for customer-owned business logic, plus WASM for bounded third-party extensions.
- Official batteries for CMS, commerce, memberships, events, admin, media, auth, jobs, and ops.
- A deployable customer-app story, not just a library crate story.
- A path from local Docker dev to production operations that stays coherent.

## What You Get

- A customer-root Rust app model instead of a framework that stops at HTTP handlers.
- HTML-first rendering with progressive enhancement layered on where it adds value.
- Multi-site, multi-locale, and market-aware product boundaries without forcing a headless architecture.
- Built-in batteries for CMS, commerce, events, memberships, media, admin, jobs, auth, and ops.
- A first-party path for customer-owned Rust business logic and a separate WASM path for bounded third-party extensions.

## What Is In This Repo

- `crates/`
  Davenda core crates and official modules.
- `apps/shoppr/`
  The reference multi-market ecommerce starter.
- `apps/gitly/`
  A non-commerce demo showing the platform can also power developer products and content-heavy web apps.
- `docs/design/`
  Architecture records and internal design chapters.
- `website/`
  The public Docusaurus documentation site.

## Start In Minutes

The fastest way to understand Davenda is to run the product, not just read about it.

### 1. Run Shoppr

```bash
cd apps/shoppr
cp .env.example .env
docker compose -f docker-compose.yml -f docker-compose.repo.yml up --build
```

Then open:

- `http://uk.localhost:8080/`
- `http://fr.localhost:8080/`
- `http://pl.localhost:8080/`
- `http://localhost:8080/__dev`

### 2. Read The Public Docs

```bash
cd website
npm install
npm run start
```

That gives you:

- getting started docs for Rust web developers
- Shoppr-led ecommerce walkthroughs
- Gitly-led general web app walkthroughs
- operations and deployment guidance
- architecture chapters surfaced as a separate subsite

## Why The Demos Matter

Davenda is easier to understand through believable products than through isolated crates.

- `Shoppr` shows the ecommerce path: markets, locales, merchandising, checkout, admin, and operations.
- `Gitly` shows the same runtime can power a product that is clearly not a store.

If a developer opens this repo and sees only abstractions, the framework loses the argument. The demos exist to prove the product shape.

## The Product Shape

Davenda is intentionally split into clear layers:

- Davenda core: runtime, routing, storage, cache, auth execution, jobs, observability, TLS, rendering.
- Official modules: CMS, commerce, memberships, events, admin, media, ops, and other reusable batteries.
- Customer apps: branding, templates, configuration, auth mappings, Rust business logic, and optional WASM extensions.

The demos are not toy frontends taped onto a crate. They are meant to be believable starting points for real customer work.

## What Makes Davenda Different

Most Rust web stacks stop at "we gave you an HTTP framework."

Davenda is trying to solve a different problem:

- how a team ships a whole web product in Rust
- how that product stays customizable without collapsing into a plugin free-for-all
- how HTML-first rendering, accessibility, and interactivity can coexist cleanly
- how to move from local development to production without inventing a new platform on the side

That is why the repo includes both architecture chapters and end-to-end demos.

## Learn The Framework Through Real Apps

### Shoppr

Shoppr is the ecommerce reference:

- multi-site
- multi-locale
- catalog and merchandising
- account flows
- checkout and payments
- customer-linked Rust backend logic
- third-party WASM integration boundary

### Gitly

Gitly is the versatility demo:

- repository and org-style information architecture
- theme and locale switching
- mock API surfaces
- scheduled-task-driven UX
- a product shape that is clearly not commerce, on the same platform

## Documentation Map

- Public docs site: `website/`
- Architecture and ADRs: `docs/design/`
- Shoppr walkthroughs: `website/docs/use-cases/shoppr/`
- Gitly walkthroughs: `website/docs/use-cases/gitly/`
- Operations and deployment: `website/docs/operations/`
- Reference and composition docs: `website/docs/reference/`

## Public Launch Surfaces

This repo now includes the standard OSS surfaces teams expect before they try a project:

- public docs site
- contribution guide
- code of conduct
- security policy
- issue templates
- pull request template
- GitHub Actions for CI, docs publishing, and release automation
- support, security, and contribution policies

## Contributing

If you want to help shape Davenda:

- open issues with concrete product or platform gaps
- send PRs with tests and documentation
- challenge weak assumptions in the architecture docs
- contribute from the perspective of building real web products in Rust

Start with [CONTRIBUTING.md](CONTRIBUTING.md).

## Status

Davenda is ambitious on purpose. It is opinionated, product-focused, and optimised for teams that want a coherent Rust web platform rather than a thin request router.

If that is what you are looking for, start with Shoppr and the public docs. They are the shortest route from curiosity to understanding.

## License

Davenda is available under the [MIT License](./LICENSE).
