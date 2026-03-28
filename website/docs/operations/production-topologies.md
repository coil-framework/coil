---
title: Production Topologies
---

Davenda supports multiple deployment shapes, but they should all preserve the same operator
contract.

## What Is This?

This page explains the practical production topologies a Davenda app can use and how the checked-in
apps map to them.

## Why Does Topology Matter?

Topology determines:

- how you cut traffic over
- how assets are delivered
- how workers run
- how sidecars and integrations are separated
- what failure domains operators need to reason about

## Topology 1: Single Customer Binary

This is the cleanest model:

- one customer binary
- one app root
- official modules linked into that binary
- linked customer Rust compiled into the same binary

This is the default mental model for Davenda.

## Topology 2: Containerized Customer Runtime

This is the practical deployment shape demonstrated by the checked-in apps.

Concrete examples:

- `apps/shoppr/docker-compose.yml`
- `apps/gitly/docker-compose.yml`

Benefits:

- immutable runtime image
- predictable dependency surface
- easier local/staging/prod parity

## Topology 3: Customer Runtime Plus Sidecar

Use this when a separate process boundary is genuinely useful.

Shoppr's optional loyalty backend is the canonical checked-in example:

- `apps/shoppr/backend/shoppr-loyalty-backend/`

This is the right topology when you need a real process boundary for integration or HTTP concerns,
not when you merely lack a first-party customization path.

## Topology 4: Shared Runtime Plus Workers

If the product uses jobs heavily, treat worker execution as part of the topology, not as an
implementation footnote.

The operator contract should still cover:

- worker identity
- queue inspection
- dead-letter recovery
- rollout coordination with request-serving nodes

## Multi-Site Topology

Shoppr demonstrates the important rule for multi-site deployments:

- one customer app
- one deployment surface
- multiple sites declared in config

Do not clone the app into multiple hidden mini-apps unless there is a real product boundary that
demands it.

## Same-Origin Versus CDN Asset Delivery

Topology also affects how assets are served.

- same-origin delivery is simpler
- CDN delivery scales and caches better

This should be chosen deliberately and reflected in `cdn_base_url`.

## Current Checked-In Examples

### Shoppr

Shows:

- multi-site customer app
- customer binary
- linked backend
- optional sidecar
- object-store-backed published assets

### Gitly

Shows:

- single-site multilingual customer app
- customer binary
- linked backend
- runtime-installed WASM jobs and API extension examples

## Common Mistakes

### Splitting services by habit

If a boundary is not operationally real, keep the code in the customer binary.

### Forgetting workers in topology design

Background work is part of the production system, not a postscript.

### Rebuilding multi-site as multiple cloned apps

Davenda's site model exists to avoid that drift.

## What To Read Next

- [Project organization](project-organization.md)
- [Build and deploy](build-and-deploy.md)
- [Asset publication and CDN delivery](asset-publication-and-cdn-delivery.md)
