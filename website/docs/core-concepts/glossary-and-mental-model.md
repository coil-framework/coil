---
title: Glossary And Mental Model
---

Davenda becomes much easier to reason about once the main nouns stop competing with each other.

This page gives you the minimum shared vocabulary for the rest of the documentation.

## What It Is

Davenda is a product-oriented Rust web framework built around a few explicit layers:

- core runtime primitives
- official modules
- customer applications
- linked customer Rust
- bounded third-party extensions

Those layers are deliberate. Davenda is trying to avoid the common failure mode where everything can call everything else and the architecture collapses into one large implicit application.

## Why This Model Exists

Most framework confusion starts when the same word is used for different responsibilities.

For example:

- "app" sometimes means the whole deployed product
- sometimes it means the binary
- sometimes it means the runtime manifest
- sometimes it means a frontend bundle

Davenda tries to be stricter than that. Once you understand the vocabulary, the decisions around composition, auth, rendering, and extension boundaries make much more sense.

## The Core Terms

### Core

Core is the platform layer that provides runtime primitives: routing, rendering, config loading, auth boundaries, jobs, storage integration, and other shared infrastructure.

Core should not quietly grow product batteries. If a capability looks like reusable domain behavior, it probably belongs in a module instead.

### Official module

An official module is a first-party reusable battery such as CMS, commerce, memberships, events, admin, media, or ops.

Modules are linked natively, versioned explicitly, and composed into the customer application by the customer binary.

### Customer application

A customer application is the actual product you are building. It owns:

- the binary
- the app manifest
- templates and theme assets
- customer-specific Rust code
- auth package files
- optional extensions

Shoppr and Gitly are both customer applications.

### Linked customer Rust

This is customer-owned Rust logic compiled into the application through stable public APIs. It is the preferred path for product-specific backend behavior.

### Third-party extension

This is lower-trust or externally supplied functionality that runs through bounded host APIs, typically using WASM. It is not the same thing as customer-owned first-party code.

### Site

A site is a first-class delivery unit inside a customer app. Different sites can map to different hosts, default locales, brand identity, and assortment.

### Locale

Locale is a request-level concern, not just a template string. It affects routing, rendering, metadata, and product presentation.

### Market

Market is a commerce-facing concept about selling conditions such as assortment, pricing, and regional availability. It is related to sites, but it is not the same thing as site identity.

## How The Pieces Fit Together

The usual flow looks like this:

1. A customer binary links the runtime plus chosen official modules.
2. The app manifest and platform config describe the application shape.
3. A request resolves against site, locale, routes, auth, and module surfaces.
4. The runtime renders HTML or executes an action using the composed application model.

That is the mental model to keep in your head while reading the rest of the docs.

## Common Mistakes

### Treating modules like ad hoc libraries

Official modules are not just helper crates. They contribute routes, policies, capabilities, templates, jobs, and data model surfaces.

### Treating customer code like a plugin

Customer-owned Rust is part of the application. It should not be mentally grouped with low-trust third-party extension code.

### Treating locale or site as theme-level concerns

In Davenda they are runtime concepts, not just presentation details.

## What To Read Next

- [Customer-root workspace](customer-root-workspace.md)
- [Runtime and module composition](runtime-and-module-composition.md)
- [Customer apps vs official modules](customer-apps-vs-official-modules.md)
