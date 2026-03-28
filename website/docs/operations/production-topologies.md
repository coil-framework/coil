---
title: Production Topologies
---

Coil supports multiple deployment shapes, but they should all preserve the same operator
contract.

## What Is This?

This page explains the practical production topologies a Coil app can use.

## Why Does Topology Matter?

Topology determines:

- how you cut traffic over
- how assets are delivered
- how workers run
- how integrations are isolated
- what failure domains operators must reason about

## Topology 1: Single Customer Binary

This is the cleanest model:

- one customer binary
- one app root
- official modules linked into that binary
- linked customer Rust compiled into the same process

Use it when:

- the product is operationally one application
- you do not need a separate process boundary for customer logic
- you want the simplest deploy and rollback story

This is the default Coil mental model.

## Topology 2: Containerized Customer Runtime

This is the same application model delivered as a container.

Use it when:

- you want immutable runtime images
- you want cleaner local/staging/prod parity
- you want infrastructure to treat the app as one deployable unit

This is often the most practical first production shape.

## Topology 3: Customer Runtime Plus Sidecar

Use a sidecar only when there is a real reason for a process boundary, for example:

- an integration has different scaling needs
- a different security posture is required
- the integration surface is operationally separate

Do not introduce a sidecar just because the team is used to doing that in every stack.

## Topology 4: Shared Runtime Plus Workers

If the application uses jobs heavily, workers are part of the topology, not a postscript.

You need to reason about:

- worker identity
- queue inspection
- dead-letter handling
- deploy and rollback coordination between serving nodes and workers

## Multi-Site Topology

For multi-site products, the Coil model is:

- one customer app
- one deployment surface
- multiple sites declared in config

Do not rebuild multi-site as three cloned apps unless there is a real business and operational
boundary that justifies it.

## Asset Delivery Topology

You also need to choose whether assets are:

- same-origin with the app
- served from object storage or a CDN

That choice affects cutover, cache behaviour, and rollback.

## Choosing Between Topologies

Use this decision guide:

- choose a single customer binary by default
- choose containers when you want operational repeatability
- add workers when background work is real
- add sidecars only when the process boundary is operationally justified
- add CDN delivery only when frontend caching and delivery needs justify it

## Supporting Repo Examples

The checked-in apps show useful supporting variants:

- Shoppr: multi-site commerce app, linked backend, optional sidecar, asset publication
- Gitly: single-site multilingual app, linked backend, runtime-installed extension examples

Those examples prove the patterns, but the topology decision rules above should still stand even if
the demo apps changed tomorrow.

## Common Mistakes

### Splitting services by habit

If a boundary is not operationally real, keep the code in the customer binary.

### Forgetting workers in topology design

Background work is part of the production system, not an implementation detail.

### Rebuilding multi-site as cloned apps

Coil's site model exists to avoid that drift.

## What To Read Next

- [Project organization](project-organization.md)
- [Build and deploy](build-and-deploy.md)
- [Asset publication and CDN delivery](asset-publication-and-cdn-delivery.md)
