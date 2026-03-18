# Security Boundaries for Customer Apps and Extensions

**Part:** Authorization and Security  
**Chapter:** 50

Customization is a supported product feature, but it is not allowed to dissolve the runtime boundary. The platform draws a clear line between what a customer app may configure, what an extension may request, and what only core may control.

## Customer App Boundary
Customer apps are first-class applications built on the platform. They choose installed modules, templates, themes, content models, locale policy, SEO content, storage rules, and the auth model package. They may define custom pages, workflows, and business rules. They do not redefine the core runtime, auth engine, cache semantics, TLS lifecycle, or storage credential model. Those remain host responsibilities because they are part of the security envelope, not cosmetic application code.

## WASM Boundary
WASM is the default customization surface for customer-specific behavior, but it is intentionally constrained. Safe host APIs include:

- authorization checks and related lookup APIs
- request and response integration for pages and endpoints
- jobs, webhooks, and admin widgets
- metadata contribution such as translations, sitemap entries, JSON-LD nodes, and cache hints
- storage operations routed through policy-aware host services

Unsafe host access stays out of bounds. Extensions must not read auth tables directly, obtain raw object-store credentials, issue certificates, alter HTTP cache semantics, or bypass secret handling. Resource limits on time, memory, outbound HTTP, storage, and secret access are part of the security model, not optional tuning.

## Refusing Unsafe Customization
The platform must say no in a few places. Core itself is never implemented as WASM. Major first-party modules are native first, even when they expose extension slots. Customer apps can replace auth semantics, but not by teaching modules to bypass capabilities. An extension may influence pricing, search indexing, or page fragments, but it does so through explicit contracts, not by direct reach into core internals.

That boundary is what keeps modularity from becoming accidental privilege escalation. A customer app can be highly customized while the system still retains one auditable authorization engine, one storage policy layer, and one trusted runtime.
