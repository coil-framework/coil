# Zanzibar-Inspired Authorization in Core

**Part:** Authorization and Security  
**Chapter:** 43

Authorization is not a late module concern in this platform. It is a core runtime service, designed from day one around a Zanzibar-style relationship model evaluated over Postgres. That choice matches the shape of the product: multi-site and brand-aware deployments, customer accounts, editorial workflows, memberships, event staff, support teams, service accounts, and customer-specific organizational rules. A flat role table would become either too weak or too hard-coded long before the rest of the framework settled.

## What Core Owns
Core owns the parts that must remain stable no matter which customer app or official modules are installed:

- tuple storage and the query engine
- model parsing and validation
- `check`, `list`, `lookup`, and `expand` style APIs
- the recursive CTE execution strategy in Postgres
- caching and invalidation hooks
- developer tooling for explaining decisions
- migration and versioning support for auth models

What core does not own is the one true set of resource types or role names. The engine is fixed; the semantics are not. That is why the default shipped behavior is a model package, not a pile of hard-coded `site#admin` assumptions inside the framework.

## Capabilities Over Roles
Official modules consume capabilities such as `cms.page.read`, `catalog.product.edit`, `admin.users.manage`, or `asset.publish`. They do not depend on relation names. The active auth model, whether the default one or a customer replacement, binds those stable capabilities to its own relationship logic. This keeps module code replaceable and makes "custom auth model" real rather than cosmetic.

The practical result is that a CMS publish action, a checkout refund flow, and an event check-in screen all use the same host authorization surface. Modules ask the engine to evaluate a capability against a resource and actor; the engine resolves the graph defined by the current model package. WASM extensions use the same host API and are deliberately denied direct reads of auth tables so that all authorization remains auditable and enforceable in one place.

## Why It Lives in Core
Putting the engine in core avoids the worst possible failure mode: every first-party module inventing its own permission system and then trying to retroactively reconcile them. The platform would lose explainability, shared tooling, and coherent security boundaries. By treating relationship auth as a foundational service, the framework gets a single authorization runtime that can serve commerce, CMS, memberships, media, and admin without locking customers into a single organization chart.
