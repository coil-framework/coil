---
title: Zanzibar And Core Auth
---

Davenda authorization is Zanzibar-inspired.

That means access is modeled as relationships between subjects and resources, not as one global role table.

Examples of relationship facts:

- a user is a member of a group
- a group administers a site
- a site editor may edit a page
- a merchandiser may featured-edit a product

## Why Davenda Uses This Model

Davenda needs one auth system that can cover:

- multi-site storefronts
- editorial publishing
- memberships and entitlements
- events and bookings
- support and finance operations
- customer-specific organization structure

A flat role table becomes brittle fast in that environment.

## What Core Owns

Core auth owns the engine-level pieces:

- tuple storage
- schema parsing and validation
- check/list/lookup style APIs
- recursive graph evaluation
- explanation tooling
- caching and invalidation

Core does not own one universal set of relation names.

That is the job of the active auth package.

## The Three-Layer Model

Think about auth in three layers:

1. `Tuple storage`
   - persistent relationship facts
2. `Auth schema`
   - resource types, relations, and derived permissions
3. `Capability bindings`
   - stable contracts that modules ask for

That split is the point. It lets the engine stay stable while semantics remain replaceable.

## Capabilities Over Roles

Official modules ask questions like:

- `cms.page.publish`
- `catalog.product.edit`
- `order.refund.issue`

They do not ask:

- is this user `site#admin`?
- does this actor have relation `editor` on this page?

The current auth package decides which schema relation satisfies the capability.

## How This Maps To Zanzibar Thinking

In Zanzibar-style terms:

- `subject`: the actor being checked
- `resource`: the object being accessed
- `relation` or `permission`: the semantic being evaluated
- `tuple`: one stored relationship fact

Davenda adds a stable capability layer on top so official modules do not couple themselves to one relation graph.

## Why This Matters For Customer Apps

This model is what makes the following possible without forking modules:

- adding customer-specific operator roles
- adding domain-specific resources
- tightening approval flows
- replacing the default organization model

If official modules depended directly on relation names, "custom auth schema" would be fake.

## Common Mistakes

- Explaining Zanzibar only as "graph auth." The important boundary is not just graph traversal; it is the separation of engine, schema, and capability contracts.
- Treating capability names as equivalent to stored tuple relations. They are not.
- Assuming customer code or WASM should read raw auth tables directly. Davenda is designed to keep authorization decisions inside the core auth service.
