---
title: Customer Apps Vs Official Modules
---

Davenda makes a hard distinction between a customer application and an official module.

That distinction is one of the reasons the framework stays coherent as the product surface grows.

## What It Is

### Official modules

Official modules are first-party reusable batteries such as CMS, commerce, memberships, events, admin, media, and ops.

They are designed to be installed into many customer apps.

### Customer applications

Customer applications are the actual products built with Davenda. They decide:

- which modules are linked
- which modules are enabled
- which sites and locales exist
- which templates, theme assets, and product-specific behaviours define the application

## Why This Distinction Exists

Without it, reusable batteries and product-specific code start to bleed into each other.

That usually creates two bad outcomes:

- the framework turns into a pile of one-off application assumptions
- product teams start forking reusable behaviour because the boundary is unclear

Davenda tries to avoid both.

## How It Works

An official module contributes reusable runtime surfaces such as:

- routes
- capabilities
- jobs
- data model elements
- admin surfaces
- integration points

A customer app then composes those modules into a concrete product and adds:

- templates and theme
- site structure
- customer-specific hooks
- product decisions that are not general-purpose batteries

## Shoppr Example

Shoppr is a customer app. It uses official modules such as:

- CMS
- media
- commerce
- memberships
- events
- admin
- ops

Those modules are enabled in:

- `apps/shoppr/app.toml`

But Shoppr itself still owns:

- its market structure
- its templates
- its theme assets
- its auth package choice
- its linked customer backend
- its runtime-installed waitlist extension

That is the intended separation.

## Gitly Example

Gitly proves the same pattern outside commerce. It is still a customer app even though the product shape is closer to a code-hosting experience than a store.

The lesson is that official modules provide reusable batteries, but the customer app still owns the actual product.

## A Useful Rule Of Thumb

Ask this question:

"Would this behaviour plausibly belong in many customer applications without being rewritten around one product's identity?"

If yes, it is a candidate for an official module.

If no, it probably belongs in the customer app.

## What Official Modules Should Own

Official modules are the right place for:

- reusable admin workflows
- reusable route surfaces
- reusable capability contracts
- reusable data contracts
- reusable jobs and integration surfaces

## What Customer Apps Should Own

Customer apps are the right place for:

- brand identity
- site and locale structure
- market-specific product decisions
- customer-specific templates and presentation
- linked Rust business rules
- runtime-installed extensions selected for that one product

## What Not To Confuse With An Official Module

Customer-owned Rust can be first-party without becoming an official reusable battery.

That means:

- linked Rust backend code is still customer code
- runtime-installed WASM is still product-selected extension behaviour
- neither of those automatically becomes an official module

## Common Mistakes

### Putting product identity into reusable modules

That makes modules harder to reuse and harder to evolve independently.

### Rebuilding shared batteries inside the customer app

That weakens the value of the module layer and tends to duplicate auth, route, and operational behaviour.

### Confusing customer Rust with a module

Customer-owned Rust can be first-party without becoming an official reusable battery.

### Treating modules as template bundles only

They are runtime batteries, not just UI packages.

## Read Next

- [Runtime and module composition](runtime-and-module-composition.md)
- [Official modules](../reference/modules.md)
- [Customer project layout](../getting-started/customer-project-layout.md)
- [Customer Rust vs third-party WASM](../reference/customer-vs-wasm.md)
