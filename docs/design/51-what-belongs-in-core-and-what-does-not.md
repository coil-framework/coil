# What Belongs in Core and What Does Not

**Part:** Native Batteries  
**Chapter:** 51

The platform needs a hard rule here because this is where frameworks usually decay. If too much goes into core, the system turns into a bloated product bundle that every customer drags around. If too little goes into core, the framework becomes a thin shell and every module reimplements the same infrastructure badly. The right split is based on whether a concern is a cross-cutting primitive or a product distribution.

## Core
Core contains services that every serious customer app and every module depend on, regardless of domain:

- HTTP runtime, routing, middleware, configuration, observability, and testing hooks
- data access, migrations, transactions, jobs, scheduling, and events
- templating and frontend integration boundaries
- auth engine, tuple storage, model system, and capability evaluation
- cache, storage, asset publication, and related policy engines
- i18n, SEO primitives, accessibility contracts, and TLS lifecycle
- the WASM host runtime and extension ABI

These are the trusted host capabilities. They are not optional plugins and they are not implemented inside the third-party sandbox.

## Official Native Modules
Official modules are the first-party batteries included on top of core: CMS, catalog, checkout, admin shell, memberships, events, media, reporting, and similar domain packages. They are versioned separately from core and installed per customer app. They must consume core services rather than reimplement them. A module may define content workflows or booking policies; it must not invent its own auth engine, translation runtime, or storage credential layer.

## Customer Apps and Extensions
Customer apps own the parts that are specific to a customer deployment: templates, design system choices, content schema, selected module set, locale policy, SEO content, auth model choice, and any custom business logic. WASM extensions are the preferred home for isolated custom behavior such as widgets, pricing rules, webhook handlers, or page endpoints. If a feature requires deep transactional behavior, heavy data access, or platform-level control, it probably does not belong in WASM and may belong in a native module instead.

The durable rule is therefore simple: cross-cutting primitives live in core, supported product features live in native modules, customer specificity lives in the app, and sandboxed customization lives at the edge.
