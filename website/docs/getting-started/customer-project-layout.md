---
title: Customer Project Layout
---

Davenda's preferred shape is a customer-owned Rust workspace that depends on Davenda as upstream crates.

That point is structural, not cosmetic. The customer project is the composition root. It owns the binary, the app manifest, the theme, the customer backend logic, and the decision about which official modules are linked.

## What It Is

A Davenda customer project usually contains:

- one binary crate that starts the application
- one or more customer-owned Rust crates
- an application root with `app.toml`, platform config, templates, theme assets, auth package files, and optional extensions

At a high level, the layout looks like this:

```text
customer-product/
  Cargo.toml
  crates/
    my-product-app/
    my-product-backend/
    my-product-bin/
  app/
    app.toml
    platform.toml
    templates/
    theme/
    auth/
    extensions/
```

The exact folder names can vary, but the responsibilities should stay recognizable.

## How To Use This Page

Use this page when you already accept the customer-root model and need to answer:

- what folders and crates should exist
- what belongs in the app root versus the Rust workspace
- where to look next for exact `app.toml`, platform config, module composition, and deploy guidance

If you are still deciding whether linked customer Rust should exist at all, read
[Linked Rust backends](linked-rust-backends.md) next. If you need the exact architecture model
behind this layout, jump to
[Customer-root workspace](../core-concepts/customer-root-workspace.md).

## Why This Shape Exists

This layout solves a few recurring problems in Rust web applications.

### It keeps the customer binary in charge

The customer app, not the framework, decides which modules are linked, which customer plugins are registered, and how the runtime is started.

### It keeps upgrades honest

Because the customer project depends on Davenda as ordinary crates, upgrading Davenda looks like a dependency upgrade, not a hidden fork of the framework.

### It gives customer Rust a first-party path

Customer-specific backend logic does not need to hide in an external sidecar or a pile of runtime scripting. It can live in ordinary Rust crates, compiled and tested with the rest of the app.

## How It Works

There are three layers to keep straight.

### 1. The customer workspace

This is the Rust project you own. It contains your binary and your customer-specific code.

### 2. The application root

This is where Davenda's runtime-facing application inputs live:

- `app.toml`
- platform config
- templates
- theme assets
- auth package files
- optional extension artifacts

### 3. Davenda crates

These are the upstream crates that provide the runtime, official modules, customer SDK, and supporting batteries.

The customer binary ties those three layers together.

## A Concrete Example

The repo includes two customer-root examples:

- `apps/shoppr`
- `apps/gitly`

Use them to see how the workspace, app root, and runtime fit together in a real application instead of a stripped-down tutorial.

Practical follow-on pages for those examples:

- [Shoppr overview](../use-cases/shoppr/overview.md)
- [Gitly overview](../use-cases/gitly/overview.md)
- [Project organization](../operations/project-organization.md)

## Common Mistakes

### Treating the app root as the whole project

`app.toml`, templates, and theme files are important, but they are not the full customer application. The binary and linked Rust crates matter just as much.

### Treating customer code as a plugin afterthought

Davenda does support bounded extension points, but customer-owned Rust is not supposed to look like a third-party plugin. It is part of the application.

### Hiding product decisions in ad hoc startup code

If module composition, site configuration, or customer hooks are difficult to identify in the customer binary, the project shape is starting to drift.

## What To Read Next

- [Linked Rust backends](linked-rust-backends.md)
- [Customer-root workspace](../core-concepts/customer-root-workspace.md)
- [Runtime and module composition](../core-concepts/runtime-and-module-composition.md)
- [Composition and davenda-all](../reference/composition.md)
- [Build and deploy](../operations/build-and-deploy.md)
- [Configuration and secrets](../operations/configuration-and-secrets.md)
