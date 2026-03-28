---
title: Customer-Root Workspace
---

The customer-root workspace is the centre of gravity in Coil.

If you understand this concept, the rest of the framework feels much less unusual.

## What It Is

A Coil application is expected to live in a customer-owned Rust workspace. That workspace owns:

- the application binary
- customer-specific Rust crates
- the app manifest
- templates and theme assets
- auth package files
- extension artifacts

Coil is then consumed as upstream crates from that workspace.

## Why It Exists

This design solves three problems at once.

### Composition stays visible

The customer binary is where module composition and customer plugin registration happen. You do not have to guess where the real application is assembled.

### Upgrades stay ordinary

The customer app consumes Coil through dependencies rather than through a hidden fork or code generation boundary.

### Product logic stays close to the product

Templates, config, auth, and customer Rust all live under one application root instead of being scattered across unrelated repositories by default.

## Shoppr As The Canonical Example

Shoppr is the checked-in reference customer workspace:

- `apps/shoppr/Cargo.toml`
- `apps/shoppr/app.toml`
- `apps/shoppr/platform.dev.toml`
- `apps/shoppr/crates/shoppr-bin/src/main.rs`
- `apps/shoppr/crates/shoppr-app/src/lib.rs`

That workspace demonstrates the intended model directly:

- the workspace root owns the product
- Coil is a dependency, not the workspace root
- the binary links the official battery and customer code
- the app root carries templates, auth, theme assets, and extensions

## What The Shoppr Workspace Looks Like

This is the important part of the Shoppr tree:

```text
apps/shoppr/
  Cargo.toml
  app.toml
  platform.dev.toml
  platform.toml
  auth/
  templates/
  theme/
  extensions/
  crates/
    shoppr-app/
    shoppr-backend/
    shoppr-bin/
  backend/
    shoppr-loyalty-backend/
```

### What Each Part Is For

- `Cargo.toml`
  - the customer-owned workspace root
  - defines which crates belong to the product
- `app.toml`
  - the product manifest
  - declares sites, locales, theme roots, auth package, modules, and installed extensions
- `platform.dev.toml` and `platform.toml`
  - runtime wiring for local and production-shaped environments
- `auth/`
  - file-backed auth package definitions
- `templates/`
  - customer app templates
- `theme/`
  - theme assets and theme-owned presentation artifacts
- `extensions/`
  - runtime-installed WASM extension packages
- `crates/shoppr-app`
  - the customer app bootstrap layer
  - loads manifest, config, auth, extensions, and runtime plan
- `crates/shoppr-backend`
  - the linked first-party customer Rust hooks
- `crates/shoppr-bin`
  - the executable binary developers actually run
- `backend/shoppr-loyalty-backend`
  - a more specialised customer backend example living in the same workspace family

## How It Works

At runtime, the customer workspace contributes three kinds of input.

### 1. Rust composition

The customer binary links:

- the Coil runtime
- whichever official modules are desired
- customer-owned backend crates

In Shoppr, the binary lives here:

- `apps/shoppr/crates/shoppr-bin/src/main.rs`

The binary exposes commands such as:

- `shoppr describe`
- `shoppr validate`
- `shoppr assets publish`
- `shoppr migrate apply`
- `shoppr serve`

That is the practical benefit of the customer-root model: the app binary is the product-facing operational entrypoint, not an opaque generic host.

### 2. Application manifest and config

The app manifest and platform config describe:

- enabled modules
- site and locale structure
- auth package location
- theme and template roots
- operational settings

In Shoppr, these are:

- `apps/shoppr/app.toml`
- `apps/shoppr/platform.dev.toml`
- `apps/shoppr/platform.toml`

### 3. Customer-owned assets and templates

These define the public and admin presentation layer of the application.

That means:

- templates under `templates/`
- assets under `theme/`
- auth package files under `auth/`
- runtime-installed extensions under `extensions/`

The result is a framework where the product is not an afterthought sitting on top of a generic server skeleton.

## What The Bootstrap Layer Actually Does

Shoppr’s bootstrap layer lives here:

- `apps/shoppr/crates/shoppr-app/src/lib.rs`

That file is worth reading because it makes the customer-root model real.

Its responsibilities include:

- finding the workspace root
- loading `app.toml`
- loading `platform.*.toml`
- loading the selected auth package from `auth/`
- resolving runtime-installed extensions from `extensions/`
- registering linked customer plugins
- building the Coil runtime plan

This is the practical shape you should copy for your own application. Your app-specific naming and hooks will differ, but the ownership pattern should look similar.

## A Minimal Mental Model

Think of the customer-root workspace in layers:

1. Workspace root
   - owns the application as a Rust project
2. Customer app crate
   - turns files and dependencies into a runtime plan
3. Customer binary
   - gives developers and operators commands to run
4. Linked customer backend crates
   - add first-party business rules
5. App root files
   - declare sites, locales, templates, auth, and extensions

If any of those layers are hard to find, the workspace is drifting toward unnecessary indirection.

## What A Healthy Workspace Looks Like

A healthy customer-root workspace makes these things easy to find:

- the binary entrypoint
- the app root
- the linked backend crate
- the chosen official modules
- any optional extensions

If those are difficult to identify, the workspace is probably drifting toward unnecessary indirection.

## Common Mistakes

### Hiding the app root behind tooling

The app manifest, templates, and auth package should remain visible and ordinary. Do not over-abstract them away.

### Letting the binary become opaque

If the binary no longer clearly shows which modules and plugins are linked, the composition story is getting weaker.

### Splitting first-party product logic into unnecessary services

Some service boundaries are real. Many are just workarounds for a weak application composition model. Coil is trying to avoid the latter.

### Treating the workspace root as if it were only a Cargo convenience

In Coil, the workspace root is a product boundary. It is where the application’s shape becomes visible.

## Read Next

- [Customer project layout](../getting-started/customer-project-layout.md)
- [Runtime and module composition](runtime-and-module-composition.md)
- [Customer apps vs official modules](customer-apps-vs-official-modules.md)
- [app.toml](../reference/app-toml.md)
