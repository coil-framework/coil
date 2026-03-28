# OctoHub

OctoHub is a checked-in customer app that makes Davenda look like a GitHub-style forge instead of
a storefront. It is intentionally static-but-honest: there is no real git engine underneath, but
there are enough repositories, users, organizations, workflow runs, API responses, and extension
points to show that the framework is modular.

It demonstrates:

- customer-owned templates and theme assets
- a linked Rust backend crate
- bounded runtime-installed WASM packages
- custom GitHub-style API routes
- mock scheduled jobs that simulate Actions
- multilingual frontend copy in `en-GB`, `fr-FR`, and `de-DE`
- light, dark, and system theme switching from the frontend

## Quick Start

From this folder:

```bash
./scripts/prepare-local-dev.sh
docker compose up --build
```

If those ports collide with another local stack, copy `.env.example` to `.env` and change the
published host ports before running Compose.

Then open:

- `http://localhost:8080/`
- `http://localhost:8080/explore`
- `http://localhost:8080/octocorp/platform-ui`
- `http://localhost:8080/octocorp/platform-ui/pulls`
- `http://localhost:8080/octocorp/platform-ui/actions`
- `http://localhost:8080/orgs/octocorp`
- `http://localhost:8080/alexmariner`
- `http://localhost:8080/fr`
- `http://localhost:8080/de`

What you should see:

- a GitHub-like shell with accessible navigation
- theme switching between light, dark, and system
- language switching between English, French, and German
- static repository, organization, and profile surfaces
- custom API-driven summary cards
- a WASM-backed community pulse endpoint
- a mock scheduled-job surface for GitHub Actions

## What Lives Where

`app.toml`
- customer app identity, locales, modules, and installed WASM packages

`platform.dev.toml`
- local runtime config for HTTP, database, Redis, storage, jobs, and observability

`templates/octohub/`
- the public GitHub-style pages

`theme/assets/site.css`
- the GitHub-inspired light/dark design system and responsive layout

`theme/assets/site.js`
- locale switching, theme switching, and client-side API hydration

`crates/octohub-backend/`
- the linked customer-owned Rust backend

`crates/octohub-app/`
- customer app composition, custom route mounting, extension loading, and runtime bootstrap

`crates/octohub-bin/`
- the `octohub` customer binary

`extensions/octohub-community-pulse/`
- a bounded WASM API extension

`extensions/octohub-actions-scheduler/`
- a bounded WASM scheduled-job extension

## Linked Rust Backend

The first-party customer backend path is:

- `crates/octohub-backend`

It is linked directly into the customer runtime and currently provides:

- static repository, pull request, workflow, organization, and user fixture data
- GitHub-like JSON payloads for custom endpoints
- the linked plugin descriptor surfaced by the CLI
- a CMS publish guard that requires README-style pages to mention accessibility guidance

Useful inspection commands:

```bash
cargo run -p octohub -- describe
cargo run -p octohub -- linked-backend describe
cargo run -p octohub -- linked-backend repository
cargo run -p octohub -- linked-backend pulls
cargo run -p octohub -- linked-backend workflows
cargo run -p octohub -- linked-backend organization
cargo run -p octohub -- linked-backend user
```

## Third-Party WASM

OctoHub also demonstrates the bounded third-party path:

- `octohub-community-pulse`
  - contributes to `/api/github/pulse`
- `octohub-actions-scheduler`
  - fulfils the `github.actions.refresh` scheduled-job slot

These packages are runtime-installed, hash-pinned in `app.toml`, and intentionally narrower than
the linked Rust backend. That is the split Davenda is supposed to show:

- linked Rust for first-party customer logic
- WASM for bounded runtime-installed behavior

## API Surface

OctoHub exposes GitHub-style demo endpoints:

- `/api/github/repository`
- `/api/github/pulls`
- `/api/github/workflows`
- `/api/github/org`
- `/api/github/user`
- `/api/github/pulse`

The first five are mounted by the customer app in Rust. The last one is fulfilled through the WASM
extension boundary.

## Running Without Docker

If you already have Postgres, Redis, and object storage available:

```bash
./scripts/prepare-local-dev.sh
cargo run -p octohub -- validate
cargo run -p octohub -- migrate apply --dry-run
cargo run -p octohub -- up
```

`.env.example` documents the minimum local variables expected by the app.

## Reading Order

If you are new to Davenda, read these in order:

1. `app.toml`
2. `crates/octohub-app/src/lib.rs`
3. `crates/octohub-backend/src/lib.rs`
4. `extensions/octohub-community-pulse/package.toml`
5. `extensions/octohub-actions-scheduler/package.toml`
6. `templates/octohub/`

That path shows the entire customer-app story without needing to understand the wider repo first.
