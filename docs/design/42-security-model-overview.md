# Security Model Overview

**Part:** Authorization and Security  
**Chapter:** 42

The platform security model starts from one architectural choice: the trusted runtime is native host code, while customization runs at the edge through constrained contracts. Core is responsible for request handling, data access, auth evaluation, storage policy, secret handling, and extension isolation. Official modules are trusted code, but they are still expected to consume the same capability and service contracts that customer apps and extensions see. WASM exists specifically so that customer-specific behavior can be added without turning the framework itself into a plugin sandbox pretending to be a core runtime.

## Trust Boundaries
There are four security tiers:

- Core is the root of trust. It owns the auth engine, tuple store, secrets, TLS lifecycle, cache semantics, and storage credentials.
- Official native modules are privileged application code, but they must not bypass core contracts. They depend on capabilities, host services, and explicit registration rather than direct table or secret access.
- Customer apps control templates, content models, installed modules, locale policy, SEO content, storage rules, and auth model selection. They do not get to redefine the runtime boundary itself.
- WASM extensions are the least-trusted code. They receive a stable host ABI with capability checks, constrained storage APIs, and explicit resource limits for time, memory, outbound HTTP, storage, and secrets access.

## Least Privilege in Practice
Least privilege is enforced through capabilities rather than through ambient authority. A module or extension does not assume that "admin" means anything in a given deployment. It asks for a concrete capability such as `cms.page.publish`, `asset.manage_storage`, or `admin.users.manage`, and the active auth model decides whether the actor is allowed. The same rule applies to data and infrastructure: extensions can request storage writes or emit metadata through host APIs, but they do not receive raw object-store credentials, direct auth table access, or unrestricted certificate management.

The storage layer also carries security meaning. Delivery mode and sensitivity are explicit policy dimensions, so "public", "signed", "proxied", and "local only" are modeled decisions rather than ad hoc flags. That matters for both enforcement and auditability. An asset may live in object storage but still require signed delivery, and publication remains an auth-governed state transition rather than a side effect of where the file is stored.

## Auditability
Strong boundaries are only useful if operators can see them working. Authorization decisions are explainable, tuple writes are auditable, and privileged state transitions such as publishing content, refunding an order, or exposing a media asset must be attributable to an actor, a capability, and a model version. The platform is designed so that failures become diagnosable policy decisions, not opaque support tickets or silent bypasses.
