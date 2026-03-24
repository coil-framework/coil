# The Problem We Are Solving

The existing WordPress installation is useful as evidence, not as a target architecture. It proves that the product is no longer a simple marketing site or a thin theme around editorial content. The current business shape includes commerce, memberships, subscriptions, bookings, events, branded experiences, region-aware behavior, admin workflows, media handling, and operational integrations. In other words, the current site behaves like an application platform that happens to be implemented inside WordPress.

That matters because the current runtime pays a large cost to preserve WordPress conventions that are no longer helping. A large amount of PHP is bootstrapped on every request, the extension surface is broad and implicit, and the boundaries between content concerns, runtime concerns, and business logic are weak. The result is a system that consumes significant server memory for workloads that are still mostly server-rendered HTML, forms, and account flows. The platform is expensive not because the business problem is impossible, but because the current stack carries too much historical overhead.

The rewrite therefore has two goals at once. First, it replaces the current customer implementation with a system that is operationally leaner, easier to reason about, and better aligned with bookings, memberships, and commerce. Second, it creates a reusable product platform that can support more than one customer without collapsing into a fork-per-customer mess or a giant shared application full of branching logic. The greenfield target is a platform plus customer apps, not a one-off site rebuild.

This distinction drives the rest of the design. The platform must provide a strong native core, official first-party batteries for common product domains, and separate customer apps that compose those pieces into deployable products. It must also make customization possible without recreating WordPress-style sprawl. That is why extension boundaries, authorization, storage policy, caching, internationalization, SEO, TLS, and operational diagnostics are all first-class design topics rather than implementation details to be deferred.

The practical problem statement is therefore wider than "replace WordPress." We need a system that:

- serves HTML-first customer sites with selective interactivity and good SEO
- runs commerce, memberships, events, bookings, and CMS workflows in one coherent runtime
- uses materially less memory and less incidental infrastructure than the current stack
- supports separate customer apps on top of shared core contracts and official modules
- keeps authorization, storage, caching, observability, and deployment behavior explicit
- allows controlled extension through WASM without making the platform itself a plugin host in disguise

Seen this way, the problem is not simply technical debt. It is product shape mismatch. WordPress has become the container for a multi-domain application platform, but it does not provide the execution model, modularity, or operational control that this product family now needs. The rewrite exists to correct that mismatch with a Rust platform designed for this class of workload from day one.
