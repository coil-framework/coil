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

## What This Means In Practice

If you have used role-based systems before, the important shift is:

- you are not asking "what global role does this user have?"
- you are asking "what relationship chain connects this subject to this resource and permission?"

That is a better fit for Davenda's target workloads because sites, pages, products, bookings, memberships, and admin surfaces do not all share one flat ownership model.

## Why Davenda Uses This Model

Davenda needs one auth system that can cover:

- multi-site storefronts
- editorial publishing
- memberships and entitlements
- events and bookings
- support and finance operations
- customer-specific organization structure

A flat role table becomes brittle fast in that environment.

## When To Use This Mental Model

Use the Zanzibar model when you need to reason about:

- cross-site admin and editorial access
- inherited permissions from site to page or collection to product
- group-based access instead of only direct user grants
- customer-specific roles that should not leak into module code

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

## What Davenda Adds On Top

Davenda does not expose Zanzibar-style relations directly as the module contract.

It adds:

- capability names as the stable module boundary
- package-selected schema semantics
- explain tooling for operational debugging

That is the main difference between "graph auth exists internally" and "graph auth is safe to build a framework on."

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

## Concrete Example

A CMS publish screen does not need to know the customer's custom org chart.

It asks for a capability like:

- `cms.page.publish`

Then the active auth package decides whether that resolves through:

- `page#publisher`
- `site#admin`
- a customer-specific approval group
- some other derived permission path

That separation is what keeps official modules reusable.

## How This Maps To Zanzibar Thinking

In Zanzibar-style terms:

- `subject`: the actor being checked
- `resource`: the object being accessed
- `relation` or `permission`: the semantic being evaluated
- `tuple`: one stored relationship fact

Davenda adds a stable capability layer on top so official modules do not couple themselves to one relation graph.

## Full Implementation

The Zanzibar-inspired engine and package boundary show up in these repo areas:

- `crates/davenda-auth/`
- `apps/shoppr/auth/shoppr-auth/package.toml`
- `apps/shoppr/auth/shoppr-auth/model.auth`
- `apps/shoppr/auth/shoppr-auth/capabilities.toml`

## Why This Matters For Customer Apps

This model is what makes the following possible without forking modules:

- adding customer-specific operator roles
- adding domain-specific resources
- tightening approval flows
- replacing the default organization model

If official modules depended directly on relation names, "custom auth schema" would be fake.

## Common Developer Mistake

A common mistake is trying to model auth as if every action needs a new module-side role.

The better Davenda pattern is:

1. keep the module capability stable
2. change the auth package semantics behind it

That keeps customer policy flexible without turning every official module into a customer-specific fork.

## Common Mistakes

- Explaining Zanzibar only as "graph auth." The important boundary is not just graph traversal; it is the separation of engine, schema, and capability contracts.
- Treating capability names as equivalent to stored tuple relations. They are not.
- Assuming customer code or WASM should read raw auth tables directly. Davenda is designed to keep authorization decisions inside the core auth service.

## Read Next

- [Auth Packages](./auth-packages.md)
- [Auth Schema](./auth-schema.md)
