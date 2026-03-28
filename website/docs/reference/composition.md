---
title: Composition And davenda-all
---

Davenda supports two composition styles:

- a convenience battery through `davenda-all`
- explicit selection of the exact subsystems and official modules a customer binary links

Both are valid. They serve different stages of adoption.

## What `davenda-all` Is

`davenda-all` is the convenience distribution for teams that want the standard official stack
available without selecting every crate individually.

Use it when:

- you are learning the platform
- you want the shortest path to a believable customer app
- you expect to use most official batteries
- you do not yet need to optimize the binary surface or dependency graph aggressively

It is the easiest way to start a serious Davenda product quickly.

## What `davenda-all` Is Not

It is not:

- a replacement for the customer app manifest
- a signal that all modules are installed automatically
- a reason to stop reasoning about what your product actually links and enables

Even with `davenda-all`, the customer app manifest still decides what the runtime installs.

## When To Compose Explicitly

Prefer explicit composition when:

- you are building a specialized product with a narrower domain surface
- you want tighter control over the binary and transitive dependency set
- you need to reason precisely about which operator workflows and module contracts should exist
- you are building a long-lived platform product with stricter release-management requirements

This is often the right choice once a team moves beyond initial adoption.

## The Two Layers Of Composition

Davenda composition happens at two different layers:

### Binary Composition

The customer binary decides which official modules and customer-owned code are linked into the
runtime at all.

### Runtime Installation

The customer app manifest decides which of those available modules are installed for the specific
product and environment.

That means “linked” and “enabled” are not the same thing.

## Recommended Mental Model

Think of `davenda-all` as:

- broad compile-time availability

Think of the customer app manifest as:

- runtime product definition

Think of the customer binary as:

- the deployable contract between those two layers

## Typical Paths

### Fastest Path

Use `davenda-all`, then:

- start with a reference customer app
- trim the manifest to the modules you actually want
- add customer-owned templates, auth, and linked Rust logic

This gives teams the fastest believable start.

### Controlled Product Path

Link only the crates you intend to support, then:

- keep the manifest narrow
- validate the auth package against that module set
- document the operator surface that follows from those modules

This is better when product scope is already clear.

## How To Choose

Ask these questions:

- are we still learning the platform or already narrowing a long-term product
- do we expect to use most official modules or only a focused subset
- is binary size or dependency control already important
- do we want convenience now or precision now

If the honest answer is “we need to move quickly and see the whole product shape,” start with
`davenda-all`.

If the honest answer is “we already know the system we want to ship,” compose explicitly.

## A Note On Examples

The checked-in customer apps in this repo are intentionally part of the teaching story:

- the reference ecommerce app shows the broader product shape
- the non-commerce example shows the runtime is not tied only to storefronts

Developers should be able to understand from those examples:

- what comes from core
- what comes from official modules
- what comes from customer-owned composition

That clarity matters more than whether a project uses the convenience battery or the narrow path.
