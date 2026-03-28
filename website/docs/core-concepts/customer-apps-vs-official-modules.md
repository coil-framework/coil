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
- which templates, theme assets, and product-specific behaviors define the application

## Why This Distinction Exists

Without it, reusable batteries and product-specific code start to bleed into each other.

That usually creates two bad outcomes:

- the framework turns into a pile of one-off application assumptions
- product teams start forking reusable behavior because the boundary is unclear

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

## A Useful Rule Of Thumb

Ask this question:

"Would this behavior plausibly belong in many customer applications without being rewritten around one product's identity?"

If yes, it is a candidate for an official module.

If no, it probably belongs in the customer app.

## Common Mistakes

### Putting product identity into reusable modules

That makes modules harder to reuse and harder to evolve independently.

### Rebuilding shared batteries inside the customer app

That weakens the value of the module layer and tends to duplicate auth, route, and operational behavior.

### Confusing customer Rust with a module

Customer-owned Rust can be first-party without becoming an official reusable battery.

## What To Read Next

- [Runtime and module composition](runtime-and-module-composition.md)
- [Official modules](../reference/modules.md)
- [Customer project layout](../getting-started/customer-project-layout.md)
