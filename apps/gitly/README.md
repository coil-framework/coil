# Gitly

Gitly is a checked-in customer app that makes Coil look like a GitHub-style forge instead of
a storefront. It is intentionally static-but-honest: there is no real git engine underneath, but
there are enough repositories, users, organizations, workflow runs, API responses, and extension
points to show that the framework is modular.

It demonstrates:

- customer-owned templates and theme assets
- a linked Rust backend crate
- bounded runtime-installed WASM packages
- custom GitHub-style API routes
- a real scheduled-job extension contract plus runtime-derived scheduler state on the Actions page
- localized routes plus client-side multilingual frontend copy in `en-GB`, `fr-FR`, and `de-DE`
- light, dark, and system theme switching from the frontend with local persistence

## Quick Start

From this folder:

```bash
./scripts/prepare-local-dev.sh
docker compose up --build
```

If those ports collide with another local stack, set the published host ports through Compose env
vars before running:

- `GITLY_HTTP_PORT`
- `GITLY_POSTGRES_PORT`
- `GITLY_REDIS_PORT`
- `GITLY_MINIO_PORT`
- `GITLY_MINIO_CONSOLE_PORT`

Then open:

- `http://gitly.localhost:58080/`
- `http://gitly.localhost:58080/explore`
- `http://gitly.localhost:58080/forgeflow/platform-ui`
- `http://gitly.localhost:58080/forgeflow/platform-ui/issues`
- `http://gitly.localhost:58080/forgeflow/platform-ui/pulls`
- `http://gitly.localhost:58080/forgeflow/platform-ui/actions`
- `http://gitly.localhost:58080/orgs/forgeflow`
- `http://gitly.localhost:58080/alexmariner`
- `http://gitly.localhost:58080/fr`
- `http://gitly.localhost:58080/de`

Those `*.localhost` hosts resolve locally without external DNS or `/etc/hosts` edits, so the
single-site Gitly stack stays self-contained in the same way Shoppr's multi-site local setup does.

What you should see:

- a GitHub-like shell with accessible navigation
- theme switching between light, dark, and system
- language switching between English, French, and German
- static repository, organization, and profile surfaces
- custom API-driven summary cards
- a WASM-backed community pulse endpoint
- a runtime-derived scheduled-job surface for GitHub Actions

## What Lives Where

`app.toml`
- customer app identity, locales, modules, and installed WASM packages

`platform.dev.toml`
- local runtime config for HTTP, database, Redis, storage, jobs, and observability

`templates/gitly/`
- the public GitHub-style pages

`theme/assets/site.css`
- the GitHub-inspired light/dark design system and responsive layout

`theme/assets/site.js`
- locale switching, theme switching, client-side API hydration, and the customer-owned translation
  dictionary used by the demo shell

`crates/gitly-backend/`
- the linked customer-owned Rust backend

`crates/gitly-app/`
- customer app composition, custom route mounting, extension loading, and runtime bootstrap

`crates/gitly-bin/`
- the `gitly` customer binary

`extensions/gitly-community-pulse/`
- a bounded WASM API extension

`extensions/gitly-actions-scheduler/`
- a bounded WASM scheduled-job extension

## Linked Rust Backend

The first-party customer backend path is:

- `crates/gitly-backend`

It is linked directly into the customer runtime and currently provides:

- static repository, pull request, workflow, organization, and user fixture data
- GitHub-like JSON payloads for custom endpoints
- the linked plugin descriptor surfaced by the CLI
- a CMS publish guard that requires README-style pages to mention accessibility guidance

Useful inspection commands:

```bash
cargo run -p gitly -- describe
cargo run -p gitly -- linked-backend describe
cargo run -p gitly -- linked-backend repository
cargo run -p gitly -- linked-backend pulls
cargo run -p gitly -- linked-backend workflows
cargo run -p gitly -- linked-backend organization
cargo run -p gitly -- linked-backend user
```

## Third-Party WASM

Gitly also demonstrates the bounded third-party path:

- `gitly-community-pulse`
  - contributes to `/api/github/pulse`
- `gitly-actions-scheduler`
  - fulfils the `github.actions.refresh` scheduled-job slot

The important bound to understand is:

- the scheduled-job contract and installed extension are real
- the workflow rows are still fixture data, so the demo stays understandable without pretending to
  be a full automation product

These packages are runtime-installed, hash-pinned in `app.toml`, and intentionally narrower than
the linked Rust backend. That is the split Coil is supposed to show:

- linked Rust for first-party customer logic
- WASM for bounded runtime-installed behaviour

## API Surface

Gitly exposes GitHub-style demo endpoints:

- `/api/github/repository`
- `/api/github/pulls`
- `/api/github/workflows`
- `/api/github/org`
- `/api/github/user`
- `/api/github/pulse`

The first five are mounted by the customer app in Rust. The last one is fulfilled through the WASM
extension boundary.

## Local Host And Asset Notes

The default local stack publishes:

- the app on `gitly.localhost:58080`
- MinIO asset delivery on `localhost:9002`
- the MinIO console on `localhost:9003`

Theme assets are published through the configured CDN base URL, so in local development you should
expect hashed CSS and JS asset URLs under `http://localhost:9002/gitly/...`.

## Running Without Docker

If you already have Postgres, Redis, and object storage available:

```bash
./scripts/prepare-local-dev.sh
cargo run -p gitly -- validate
cargo run -p gitly -- migrate apply --dry-run
cargo run -p gitly -- up
```

For direct local runs, the important runtime inputs are still the same ones used by Compose:

- `DATABASE_URL`
- `REDIS_URL`
- `OBJECT_STORE_URL`

## Reading Order

If you are new to Coil, read these in order:

1. `app.toml`
2. `crates/gitly-app/src/lib.rs`
3. `crates/gitly-backend/src/lib.rs`
4. `extensions/gitly-community-pulse/package.toml`
5. `extensions/gitly-actions-scheduler/package.toml`
6. `templates/gitly/`

That path shows the entire customer-app story without needing to understand the wider repo first.
