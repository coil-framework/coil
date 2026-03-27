# Workspace and Crate Layout

The workspace should make the platform's architectural boundaries visible in code. If the crate layout hides ownership, the implementation will drift toward accidental coupling long before the documentation is updated. The goal is not maximal granularity, but a structure that reflects the runtime, the official batteries, and the customer app composition model.

## Core Crates

Core should be split into focused crates around stable responsibilities. A reasonable shape is:

- `core-runtime` for startup, boot orchestration, lifecycle management, and top-level composition
- `core-http` for server integration, routing, middleware, request context, and response primitives
- `core-template` for the HTML-first engine, fragment rendering, and template compilation or caching
- `core-auth` for the Zanzibar-inspired engine, capability checks, model loading, and explain tooling
- `core-data` for database access primitives, transactions, migrations, and query infrastructure
- `core-cache` for cache contracts, tag and scope handling, and adapters for Moka, Redis, and Valkey
- `core-storage` for storage drivers, delivery modes, sync policy, and signed access support
- `core-assets` for deploy-asset publication, manifest handling, and CDN-oriented asset resolution
- `core-tls` for certificate lifecycle, ACME flows, Cloudflare integration, and transport policy
- `core-i18n`, `core-seo`, and `core-a11y` for the cross-cutting services that every module consumes
- `core-observability` for structured logging, metrics, tracing, and health endpoints
- `core-wasm` for extension packaging, sandbox execution, host APIs, and capability-gated host calls

This split should be treated as a guide, not a mandate to create tiny crates prematurely. Some concerns may begin co-located and separate later. What matters is that their ownership is clear and their public surfaces stay deliberate.

## Contract and SDK Crates

Shared contract crates are useful where both native modules and extensions need the same types. Capability names, host API request and response types, extension manifests, and some domain-neutral resource identifiers all belong here. These crates should remain narrow and version carefully because they are part of the compatibility story between core, official modules, and customer apps.

## Official Module Crates

Official batteries should live in their own crates or crate families, such as `module-cms`, `module-admin`, `module-catalog`, `module-checkout`, `module-membership`, `module-events`, and `module-media`. Large domains may split further when there is a real boundary, for example separate catalog, checkout, orders, and payments crates. The key rule is that modules depend on stable core contracts, not on the internal details of sibling modules.

## Customer App Crates

Each customer app should have its own top-level application crate or binary package, and the
preferred long-term model is that the customer project becomes the real workspace root while
Davenda is consumed as a normal dependency. That package selects official modules, provides
configuration, owns templates and theme assets, binds capabilities to the chosen authorization
model, and registers any customer-specific native adapters or extension packages. Treating the
customer app as a first-class package reinforces that it is a real product layer, not a
configuration folder buried inside the framework.

The workspace layout is therefore one of the platform's first guardrails. It reminds contributors that core, official modules, and customer apps are different things, and it keeps "just import it directly" from becoming the default answer to every integration problem.
