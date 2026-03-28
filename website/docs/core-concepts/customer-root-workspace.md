---
title: Customer-Root Workspace
---

The customer-root workspace is the center of gravity in Davenda.

If you understand this concept, the rest of the framework feels much less unusual.

## What It Is

A Davenda application is expected to live in a customer-owned Rust workspace. That workspace owns:

- the application binary
- customer-specific Rust crates
- the app manifest
- templates and theme assets
- auth package files
- extension artifacts

Davenda is then consumed as upstream crates from that workspace.

## Why It Exists

This design solves three problems at once.

### Composition stays visible

The customer binary is where module composition and customer plugin registration happen. You do not have to guess where the real application is assembled.

### Upgrades stay ordinary

The customer app consumes Davenda through dependencies rather than through a hidden fork or code generation boundary.

### Product logic stays close to the product

Templates, config, auth, and customer Rust all live under one application root instead of being scattered across unrelated repositories by default.

## How It Works

At runtime, the customer workspace contributes three kinds of input:

### 1. Rust composition

The customer binary links:

- the Davenda runtime
- whichever official modules are desired
- customer-owned backend crates

### 2. Application manifest and config

The app manifest and platform config describe:

- enabled modules
- site and locale structure
- auth package location
- theme and template roots
- operational settings

### 3. Customer-owned assets and templates

These define the public and admin presentation layer of the application.

The result is a framework where the product is not an afterthought sitting on top of a generic server skeleton.

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

Some service boundaries are real. Many are just workarounds for a weak application composition model. Davenda is trying to avoid the latter.

## What To Read Next

- [Customer project layout](../getting-started/customer-project-layout.md)
- [Runtime and module composition](runtime-and-module-composition.md)
- [Customer apps vs official modules](customer-apps-vs-official-modules.md)
