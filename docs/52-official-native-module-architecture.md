# Official Native Module Architecture

**Part:** Native Batteries  
**Chapter:** 52

Official modules are native packages that compile into the customer app and register themselves through explicit contracts. They are not loose plugins and they are not special-cased inside core. Their power comes from being first-party supported code built on the same host primitives that the framework exposes everywhere else.

## Module Contract
Each official module should declare, at minimum:

- its identifier and version
- dependencies on core services and other modules
- database migrations and seed/bootstrap steps
- routes, handlers, jobs, and event subscriptions
- capability requirements and any capability bindings it contributes
- admin and frontend integration points
- cache, SEO, i18n, accessibility, and storage behaviors
- extension slots exposed to WASM or customer-app code

Registration is explicit because the platform is avoiding WordPress-style hook soup. Core should know what a module contributes before the app serves traffic.

## How Modules Use Core
First-party modules are native first because they need strong integration with transactions, authorization, rendering, and debugging. They consume core services rather than replacing them. A CMS module uses the head/meta API, locale routing, and page-publish invalidation hooks. A commerce module uses the transaction system, storage abstractions, and auth capabilities. An admin module uses the accessibility-aware form and table primitives. The module boundary is real, but the infrastructure underneath is shared.

## Customer App Composition
Modules are installed per customer app, not globally across the platform. One app may ship CMS plus memberships plus events. Another may use only catalog and checkout. That is why module versioning is separate from core versioning and why module capabilities must be validated against the app's active auth model at install or startup time.

Official modules may expose WASM extension slots, but they do not cede ownership of their critical paths to the sandbox. The pattern is native spine, extension slots at the edge. That keeps first-party modules supportable while still leaving room for customer-specific behavior.
