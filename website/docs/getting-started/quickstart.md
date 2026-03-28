---
title: Quickstart
---

This quickstart is written for Rust web developers who want to bootstrap a real Coil application
before reading the deeper architecture material.

The goal is not just to "get the server running." The goal is to see the three things Coil is built around:

- the customer-root workspace shape
- HTML-first product surfaces backed by a native runtime
- explicit boundaries between customer code, official modules, and extensions

## Prerequisites

You need:

- Rust 1.85 or newer
- Docker with Compose
- Node.js 20 or newer if you also want to run the docs site locally

You do not need to understand the whole architecture before starting. Generate a real customer app
first, then read the concept pages with a working picture in mind.

## Create A Store With `cargo coil`

Install the Cargo subcommand first:

```bash
cargo install cargo-coil --locked
```

Then generate a customer workspace:

```bash
cargo coil new my-store
```

Interactive mode is the default. The wizard asks for:

- project name
- display name
- default locale
- additional locales
- official modules
- optional extra sites

When it finishes, Coil writes a customer-root workspace under `my-store/`.

## Start The Local Dependencies

The generated starter uses Postgres and Redis:

```bash
cd my-store
docker compose up -d
```

## Export The Required Environment

```bash
export DATABASE_URL=postgres://coil:coil@127.0.0.1:5432/my-store
export REDIS_URL=redis://127.0.0.1:6379/0
export COIL_COOKIE_SECRET=replace-me-with-a-long-random-secret
export COIL_CSRF_SECRET=replace-me-with-a-long-random-secret
```

## Validate And Run The Generated App

```bash
cargo run -p my-store -- validate
cargo run -p my-store -- serve
```

Open:

- `http://my-store.localhost:8080/`
- `http://www.my-store.localhost:8080/`
- `http://localhost:8080/admin`
- `http://localhost:8080/__dev`

What to inspect:

- the generated customer workspace shape
- the linked Rust backend crate
- the templates and translations
- the admin and dev surfaces
- the way the customer binary owns the app lifecycle

## Add Another Site And Locale

The generator is descriptor-backed, so you can evolve the project structure safely:

```bash
cargo coil site add eu \
  --root ./my-store \
  --display-name "EU Store" \
  --brand-name "My Store EU" \
  --canonical-domain eu.my-store.localhost \
  --default-locale fr-FR
```

```bash
cargo coil locale add pl-PL --root ./my-store --site eu
```

Then validate again:

```bash
cd my-store
cargo run -p my-store -- validate
```

This is the normal development split:

- `cargo coil` shapes the workspace
- the customer binary validates and serves the app
- the root `coil` CLI handles deeper platform operations

## Then Inspect Shoppr And Gitly

Once the generated starter feels clear, move to the richer examples.

Shoppr is the reference ecommerce app:

```bash
cd ../apps/shoppr
cp .env.example .env
docker compose -f docker-compose.yml -f docker-compose.repo.yml up --build
```

Open:

- `http://uk.localhost:8080/`
- `http://fr.localhost:8080/`
- `http://pl.localhost:8080/`
- `http://localhost:8080/__dev`

Gitly exists to show that Coil is not an ecommerce-only framework.

```bash
cd apps/gitly
cp .env.example .env
docker compose up --build
```

Use Gitly to compare the same runtime model against a different product shape. The point is not feature parity with Shoppr. The point is to see that the composition model remains the same even when the domain changes.

Open:

- `http://gitly.localhost:58080/`
- `http://gitly.localhost:58080/explore`

Like Shoppr's `*.localhost` market hosts, `gitly.localhost` resolves locally without external DNS
or `/etc/hosts` edits.

## Run The Docs Site

If you want the docs locally:

```bash
cd website
npm install
npm run start
```

## What To Look For During Evaluation

When you run the generated starter and the demos, pay attention to these questions:

- How much of the app shape came from `cargo coil`?
- Where does the customer application's own Rust code live?
- Which behaviours come from official modules versus customer code?
- How much of the public UI is plain server-rendered HTML?
- How do site, locale, and route resolution show up in the running app?
- What would need to change if you replaced the example product with your own?

Those questions matter more than whether you personally like the demo copy or visual design. Coil is about getting the product and runtime boundaries right.

## Common First-Run Mistakes

### Treating the generated workspace as disposable output

The generated workspace is part of Coil’s intended development model. `.coil/project.toml` is a
public lifecycle file, not an internal implementation detail.

### Looking only at the browser

The runtime model becomes much clearer if you also inspect the customer app workspace while it is
running.

### Expecting a single generic extension story

Coil intentionally separates:

- official modules
- linked customer Rust
- bounded WASM extensions

If you flatten those together mentally, the rest of the architecture will feel inconsistent.

## What To Read Next

- [Cargo Coil Overview](../reference/cargo-coil-overview.md)
- [Customer project layout](customer-project-layout.md)
- [Linked Rust backends](linked-rust-backends.md)
- [Glossary and mental model](../core-concepts/glossary-and-mental-model.md)
- [Customer-root workspace](../core-concepts/customer-root-workspace.md)
