# Extension Points: Pages, APIs, Jobs, Webhooks, Widgets

**Part:** Extensibility  
**Chapter:** 65

The platform's extension surface is explicit. Extensions attach to named integration points that core or official modules intentionally expose. This is a deliberate rejection of global hooks and hidden execution order. If a customer app cannot point to the registered page, API, job, webhook, or widget slot, the extension is not part of the product.

## Pages and Render Hooks

Extensions may contribute full pages, route handlers, or fragment-level render hooks. These are appropriate for customer-specific microsurfaces, campaign pages, account add-ons, and small embedded experiences.

The host still owns:

- route precedence and conflict resolution
- request parsing and response finalization
- auth and locale context
- template slot boundaries
- cache policy and HTTP semantics

This lets a customer app add custom storefront or account behavior without turning the template engine into a scripting environment.

## APIs

API extensions exist for narrowly scoped application contracts, not as a replacement for the platform's native module APIs. Each endpoint is registered explicitly and inherits host features such as auth, versioning, rate limiting, validation, and observability.

This is the right place for:

- customer-specific integration endpoints
- small read models needed by progressively enhanced UI
- bounded write flows that sit on top of official module services

It is the wrong place to reimplement catalog, checkout, CMS, or auth as a parallel framework.

## Jobs and Scheduled Tasks

Jobs are the default place for work that should not happen in the request path. Extensions may register asynchronous workflows and scheduled tasks, but the host owns queueing, retry policy, dead-letter handling, and scheduler coordination.

Jobs are particularly useful for:

- external system synchronization
- delayed notifications
- batch enrichment
- follow-up work after webhook ingestion

## Webhooks

Webhook extensions process verified events. The host handles the dangerous edges first:

- signature verification
- replay protection
- request normalization
- retry and dead-letter policy

Only then does the extension receive the event payload. This prevents each customer-specific integration from reinventing security-critical plumbing.

## Admin Widgets and Data Providers

Admin widgets are a supported extension surface for dashboards, side panels, tables, and specialized controls. They render into documented admin slots and inherit the admin shell's auth and accessibility contracts. They are intended for augmentation, not for replacing the admin shell itself.

## Composition Rule

A healthy extension point lets the host keep ownership of routing, transactions, auth, storage, and diagnostics while still giving the customer app real freedom. For example, an events app may install:

- a branded landing page extension
- a webhook consumer for an external ticketing source
- a nightly reconciliation job
- an admin widget showing waitlist pressure by event

Each piece is explicit, auditable, and removable without entangling the rest of the platform.
