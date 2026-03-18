# HTTP Server, Routing, and Middleware

The HTTP stack is designed for server-rendered applications first. Routing, middleware, and response handling should make pages, fragments, forms, redirects, and typed APIs feel equally native, without forcing everything through a JSON-first abstraction. This is one of the platform's most important differences from stacks that assume a client-heavy frontend by default.

Routing needs to express more than path matching. The runtime must support route grouping by module, site area, locale policy, auth requirements, hostname, and feature enablement. Named routes and URL generation are especially important because customer apps need stable links across CMS pages, account flows, checkout steps, admin surfaces, and redirect management. Locale-aware URL generation belongs here as well, because the runtime needs one understanding of canonical and localized paths.

The first stage of middleware deals with transport and request identity. Trusted proxy handling, forwarded header normalization, request IDs, scheme detection, and host resolution all happen before business logic. This is also where the runtime establishes whether the current request is operating under direct TLS termination or behind an external edge.

The second stage derives customer and user context. Middleware resolves the customer app, site or brand context, locale and region, session state, authenticated principal, and preview or feature-flag state. The cache layer also uses this stage to determine variation dimensions such as locale scope, public versus private visibility, and whether the response is even eligible for caching.

The third stage applies browser and policy concerns. CSRF validation, form method handling, maintenance mode, rate limiting, and auth-related gating all belong here. Importantly, middleware should enforce policies and enrich context, not hide business workflows. If a booking rule or pricing decision lives only in middleware, the platform will become hard to reason about very quickly.

Handlers should receive a typed request context that already contains the resolved application state they need: customer app identity, site context, locale, principal, trace metadata, service access, and cache policy context. That keeps handlers focused on domain work rather than reconstructing environment information from raw request parts.

The overall middleware order should be predictable and documented. A reasonable default path is transport normalization, customer-app resolution, trace context, locale and region derivation, session and principal resolution, browser policy enforcement, then response policy finalization. Consistency matters more than cleverness here. Once developers can predict how a request reaches a handler, the rest of the runtime becomes substantially easier to extend.
