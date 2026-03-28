---
title: Quickstart
---

This quickstart is written for Rust web developers who want to understand what Davenda feels like in practice before reading the deeper architecture material.

The goal is not just to "get the server running." The goal is to see the three things Davenda is built around:

- the customer-root workspace shape
- HTML-first product surfaces backed by a native runtime
- explicit boundaries between customer code, official modules, and extensions

## Prerequisites

You need:

- Rust 1.85 or newer
- Docker with Compose
- Node.js 20 or newer if you also want to run the docs site locally

You do not need to understand the whole architecture before starting. Run the examples first, then read the concept pages with a working picture in mind.

## Start With Shoppr

Shoppr is the best first stop because it exercises the most of the platform: storefront, cart, checkout, CMS, admin, auth, jobs, and operations.

```bash
cd apps/shoppr
cp .env.example .env
docker compose -f docker-compose.yml -f docker-compose.repo.yml up --build
```

Open:

- `http://uk.127.0.0.1.nip.io:8080/`
- `http://fr.127.0.0.1.nip.io:8080/`
- `http://pl.127.0.0.1.nip.io:8080/`
- `http://localhost:8080/__dev`

What to inspect:

- the public storefront pages and localised routes
- the cart and checkout flow
- the admin and CMS surfaces
- the way one customer app serves multiple sites and locales

## Then Run Gitly

Gitly exists to show that Davenda is not an ecommerce-only framework.

```bash
cd apps/gitly
cp .env.example .env
docker compose up --build
```

Use Gitly to compare the same runtime model against a different product shape. The point is not feature parity with Shoppr. The point is to see that the composition model remains the same even when the domain changes.

## Run The Docs Site

If you want the docs locally:

```bash
cd website
npm install
npm run start
```

## What To Look For During Evaluation

When you run the demos, pay attention to these questions:

- Where does the customer application's own Rust code live?
- Which behaviours come from official modules versus customer code?
- How much of the public UI is plain server-rendered HTML?
- How do site, locale, and route resolution show up in the running app?
- What would need to change if you replaced the example product with your own?

Those questions matter more than whether you personally like the demo copy or visual design. Davenda is about getting the product and runtime boundaries right.

## Common First-Run Mistakes

### Treating Shoppr as "the framework"

Shoppr is the reference ecommerce app, not the entire product model. Read it as a customer app that depends on the platform, not as the platform itself.

### Looking only at the browser

The runtime model becomes much clearer if you also inspect the customer app workspace under `apps/shoppr` or `apps/gitly` while the demos are running.

### Expecting a single generic extension story

Davenda intentionally separates:

- official modules
- linked customer Rust
- bounded WASM extensions

If you flatten those together mentally, the rest of the architecture will feel inconsistent.

## What To Read Next

- [Customer project layout](customer-project-layout.md)
- [Linked Rust backends](linked-rust-backends.md)
- [Glossary and mental model](../core-concepts/glossary-and-mental-model.md)
- [Customer-root workspace](../core-concepts/customer-root-workspace.md)
