# Design Principles and Non-Goals

The platform needs stable design rules because it is being built to outlive one customer project. The details of crate boundaries, storage adapters, or admin UI may change, but the principles below should continue to explain why the system is shaped the way it is.

## Design Principles

The rendering model is HTML first. The target workloads are storefronts, account areas, dashboards, admin forms, content pages, and booking flows. Those are best served by server-rendered HTML with strong routing, form handling, fragment rendering, and predictable cache behavior. Interactive behavior matters, but the platform treats progressive enhancement as the default and a client-heavy SPA model as the exception.

The runtime is monolith first. Core, official modules, and the selected customer app run as one coherent host process model, with jobs and external services supporting asynchronous work. This reduces operational complexity, preserves transactional clarity, and avoids inventing network boundaries where module boundaries are enough.

Core stays lean, but cross-cutting concerns still belong there. Authorization, caching, TLS, object storage policy, internationalization, SEO primitives, HTTP caching, and accessibility contracts are core services because their behavior must stay consistent across every module and customer app. By contrast, catalog management, admin CRUD surfaces, CMS workflows, and booking logic belong in official batteries because they are reusable product features rather than universal runtime primitives.

Extensibility follows the rule "native spine, WASM edge." Core is never WASM. Official modules are native first-party packages. WASM is used for controlled extension at defined points such as routes, widgets, jobs, webhooks, pricing rules, and metadata providers. This preserves modularity without freezing the core runtime behind the lowest-common-denominator extension ABI.

Authorization is capability-driven. Core owns the Zanzibar-inspired engine, tuple storage, check APIs, and developer tooling. The platform ships a default authorization model, but customer apps or developers may extend or replace it. That only works because official modules depend on capabilities, not on fixed relation names. A module should request `cms.page.publish` or `asset.read_public`, not assume that every installation has the same roles or hierarchy.

Operational clarity is a product feature. The platform is expected to own certificate lifecycle, cache behavior, storage policy, asset publication, structured logging, metrics, and traceability. If those concerns are left informal, the system will reproduce the same hidden coupling and operational guesswork that made the old stack hard to trust.

## Non-Goals

The platform is not trying to be a universal framework for every application domain. It has a clear bias toward commerce, memberships, events, CMS, media, and admin workloads. It is also not trying to provide unconstrained in-process plugin power to third-party code. The extension model is intentionally narrower than WordPress because predictability, security, and performance matter more than unrestricted dynamism.

The platform is not trying to preserve WordPress conventions, plugin APIs, or content assumptions. It is also not trying to force every customer into one default admin or one default authorization model. Where a choice exists between extreme generality and a coherent product platform for this workload, the platform should choose the narrower design with the stronger execution story.
