---
title: Project Organization
---

Davenda works best when teams keep the customer app, platform concerns, and optional extension
paths structurally separate.

This is not only a source-tree preference. It is an operational boundary.

## Recommended Top-Level Shape

A serious Davenda customer project should have clear ownership boundaries for:

- app manifest and customer-facing composition
- platform config per environment
- auth package and capability bindings
- templates and theme assets
- linked customer Rust backend crates
- optional runtime-installed WASM extension packages
- deployment and local-development scripts

The checked-in customer-root examples in this repo exist to model that shape.

## What Belongs In The Customer App

The customer app should own:

- product identity and branding
- installed module selection
- multi-site and locale policy
- templates and theme assets
- customer-specific auth package extensions
- linked Rust business logic
- optional third-party or bounded runtime-installed extensions

If it changes product behavior or customer experience, it usually belongs in the customer app.

## What Belongs In Platform Config

Platform config should own:

- runtime environment selection
- network bindings
- database, cache, jobs, and storage backends
- TLS mode and provider configuration
- observability settings
- deployment delivery and asset publication controls

If it changes how the product is operated rather than what it is, it usually belongs in platform
config.

## What Does Not Belong In The Customer App

Avoid pushing operational state into templates, content, or manifest files when it really belongs
elsewhere. Examples:

- live secrets
- ad hoc migration bookkeeping
- deployment-only host overrides that should live in environment config
- provider credentials embedded in customer code

Those shortcuts make local development feel easier and production operations much worse.

## Linked Rust Versus Runtime-Installed WASM

Treat these as different lanes, not interchangeable implementation styles.

Use linked Rust for:

- first-party customer business logic
- checkout or webhook policy tightly coupled to the customer product
- custom route or API surfaces owned by the customer team

Use runtime-installed WASM for:

- bounded third-party extensions
- replaceable integrations
- constrained runtime plugins that should not own the whole app binary

The docs and examples should make that split obvious to developers.

## Multi-Site Project Organization

For multi-site customer apps, keep the app as the composition root and sites as app-level
configuration, not separate mini-apps hidden in the same repo.

That means:

- one customer workspace
- one app manifest
- one set of module selections
- one shared deployment surface
- site-specific hosts, locales, and product visibility declared inside the app/config model

This prevents the multi-site story from collapsing into “just clone the app three times.”

## Example App Expectations

A checked-in example should be believable as a starter:

- README instructions should match actual commands
- manifests and platform configs should agree
- linked backend examples should be runnable from the customer workspace
- extension examples should reflect the supported runtime-installed path
- public pages should make key product capabilities discoverable

If the example app lies, the platform docs lose credibility.

## Team Ownership Model

The structure should support real team boundaries:

- platform team maintains reusable crates and operational standards
- customer or product team owns the customer app and release intent
- operations team owns deployment, observability, incident handling, and cutover execution

Davenda is easiest to operate when those boundaries are explicit in the project shape.
