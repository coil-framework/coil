# Fission-native application architecture

Status: accepted for the rewrite

## Decision

Coil is rebuilt as a product layer on Fission. Fission is the only UI and
application runtime: it owns retained widgets, routing, synchronous reducers,
effect declaration, typed jobs, server rendering, browser islands, full Web
applications, and static sites. Coil owns the domain and production contracts
that make those surfaces useful: site and market resolution, authorization
policy, PostgreSQL-backed repositories, transactions, durable work, media,
payments, observability, and extension boundaries.

The rewrite does not wrap Fission in a parallel component system, router, state
store, or job executor. Applications import Fission through `coil::fission` and
compose Coil domain crates directly.

## Execution model

Reducers are synchronous. A reducer or `FutureBuilder` declares a typed job;
the shell executes and awaits that job, dispatches its typed completion, and
rerenders from the resulting state. On SSR, this settling happens before the
final HTML response. A catalogue job may therefore query PostgreSQL and the
server response waits for the query without making the reducer asynchronous.

The same contract is used across three presentation shapes:

1. Public and search-facing routes use Fission SSR. They return complete,
   accessible HTML after required jobs settle.
2. A bounded interaction such as a cart drawer, search filter, or booking
   picker is a Fission island attached to its owning SSR route.
3. A dense operational product such as CMS, merchandising, fulfilment, or
   support is a full Fission Web application. It does not inherit a public-page
   DOM as an application architecture.

The documentation and marketing website is a Fission static site. Static
generation is not used for request-owned account, checkout, or operational
state.

## Request authority

`SiteRegistry` validates host ownership once and resolves a
`CoilRequestScope`. The scope carries the site, market, locale, canonical
origin, route, and session identity used by domain jobs. Unknown or ambiguous
hosts fail closed; a fallback site is not selected.

PostgreSQL remains authoritative for durable product and customer data. Fission
state is the current presentation snapshot. Caches, indexes, rendered pages,
and browser state are disposable projections and cannot prove or repair the
source of truth.

## Access boundary

One pure selector maps `CoilSessionState` and the current route to Fission's
`RouteDecision`. `ProtectedRoute` prevents a protected component tree and its
resources from being constructed before access is resolved. It is not server
authorization: every job, server action, and API independently authenticates
the request and authorizes the requested operation.

Runtime credentials are limited to runtime operations. Schema migration,
administrative provisioning, and other elevated tasks use separate operational
entrypoints and credentials.

## Delivery order

The migration proceeds by coherent vertical slices rather than preserving the
removed template runtime behind compatibility layers:

1. Establish the Fission application boundary, request scope, access selector,
   and cache variance rules.
2. Replace the public website with a Fission static site.
3. Move Shoppr public catalogue and product routes to Fission SSR with typed
   repository jobs.
4. Move cart, search, and booking interactions to bounded islands.
5. Move admin and operations to full Fission Web applications with protected
   routes and independent server authorization.
6. Remove the legacy router, template renderer, and frontend pipeline after
   their last production caller moves.

Each slice must exercise its public promise through the real Fission renderer
or browser target. Legacy code is not treated as an authority during the
migration and receives no new features.

## Visual system

The public site uses an open editorial composition: strong typography,
generous whitespace, and long rules establish hierarchy. Visible containers
are reserved for controls, overlays, alerts, and trust boundaries. Nested
rounded cards, gradients, ornamental shadows, glass effects, dashboard chrome,
and empty analytics are explicitly rejected.

First-run operational experiences lead to a useful outcome. Configuration
surfaces expose stable identities, validation or dry-run output, atomic apply,
and explicit deletion; they do not hide durable state transitions behind
decorative dashboards.
