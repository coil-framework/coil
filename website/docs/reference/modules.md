---
title: Official Modules
---

Davenda’s official modules are reusable product batteries. They are not sample code and they are
not the same thing as core.

## The Boundary

Use this rule when deciding where logic belongs:

- core owns runtime primitives and platform-wide contracts
- official modules own reusable business capability
- customer apps own product-specific composition and customization

If you blur those lines, upgrades and operational clarity become much harder.

## What Core Owns

Core and core-adjacent crates own:

- HTTP runtime and routing
- request and job execution boundaries
- config loading and validation
- rendering and template execution
- storage, cache, jobs, and TLS primitives
- auth execution and capability evaluation
- observability and operational control surfaces
- the WASM host boundary

Core should not contain product-specific commerce, CMS, or event workflows.

## What Official Modules Own

Official modules package reusable domain batteries that many customer apps can share.

Current module families include:

- CMS
- commerce
- memberships
- events
- admin
- media
- ops

These modules contribute some combination of:

- route surfaces
- HTTP surfaces
- queries and transactions
- migrations
- jobs
- admin resources
- auth capability requirements

## Module Families

### CMS

Owns content editing, publishing, routing, page and navigation workflows, preview, and editorial
constraints.

### Commerce

Owns catalog, product and collection surfaces, cart, checkout, order state, payments integration
contracts, and operator order workflows.

### Memberships

Owns subscriptions, tiers, entitlements, renewals, cancellation, and account-facing membership
state.

### Events

Owns event catalog, bookings, capacity, waitlists, check-in, and event-linked customer journeys.

### Admin

Owns reusable back-office shell primitives, widgets, and operator-facing composition surfaces.

### Media

Owns media-library domain behavior, revision/publication behavior, and media-specific policies
above the storage primitive layer.

### Ops

Owns report, search, recovery, bulk-operation, and operator workload surfaces that are specific to
business modules rather than generic runtime primitives.

## Installing Modules

A customer app does not get everything automatically.

In practice:

- the customer binary links the module crates it wants to make available
- the app manifest decides which of those linked modules are actually installed for that product
- runtime validation checks whether the configuration and auth package still match that module set

That means there are two levels of module selection:

- compile-time availability
- runtime installation

## Why This Matters

This separation makes it possible to:

- build narrower customer binaries
- keep specialized products from linking unnecessary batteries
- preserve a stable operator and release model even when products vary

## Module Selection Guidance

Use a module when:

- the capability is a reusable product battery
- it has durable contracts other products can rely on
- it belongs in the supported platform surface

Do not create a new official module just because a customer app has some unique rules. Those rules
usually belong in the customer app or a bounded extension surface.

## Modules And Auth

Modules are not only route bundles. They carry capability expectations.

A serious deployment should expect:

- module manifests to declare the capabilities they require
- auth packages to satisfy those capabilities
- validation to fail when the module/auth relationship is incoherent

That is part of what keeps Davenda installable at enterprise scale instead of drifting into hidden
cross-module assumptions.
