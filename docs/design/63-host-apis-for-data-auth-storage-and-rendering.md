# Host APIs for Data, Auth, Storage, and Rendering

**Part:** Extensibility  
**Chapter:** 63

The host API is the stable surface that makes WASM extensions useful without letting them become a second uncontrolled framework. Extensions do not talk to Postgres, Redis, the auth tables, or the object store directly. They declare intent through host APIs, and the host enforces capability checks, storage policy, caching rules, and tracing around every call.

## Data API

Data access is exposed as platform services, not raw SQL. Extensions should read and write through typed resource or module contracts owned by core or official modules. That keeps schema evolution and transaction policy in the native layer.

The data API is expected to support:

- reads against resources exposed by core or installed modules
- writes only where the extension has explicit mutation capabilities
- host-managed pagination, validation, and transactional behavior
- batched access patterns so extensions do not reintroduce N+1 behavior around auth-heavy resources

An extension can participate in business flows, but it does not get to redefine the persistence model of commerce, CMS, memberships, or events from the sandbox.

## Auth API

Auth is a first-class core service built around the capability layer. Extensions use the host API, not direct tuple access.

The stable contract should expose:

- `check(subject, action, resource)`
- `list(subject, resource_type, permission)`
- `lookup(resource, permission)`
- tuple mutation only when the extension has been granted that responsibility
- decision explain endpoints in developer or admin contexts only

Official modules depend on capabilities such as `cms.page.publish` or `catalog.product.edit`, and extensions must do the same. They should never depend on relation names from the default auth model. That is what makes custom auth models genuinely replaceable.

## Storage API

Storage is policy aware. The extension asks to read or write an object and the host resolves where it lives and how it can be served. The extension never receives raw object-store credentials.

Relevant host concerns include:

- storage class selection such as `public_upload`, `private_shared`, or `local_only_sensitive`
- delivery mode such as `public_cdn`, `signed_url`, `app_proxy`, or `local_only`
- async sync and replication behavior
- metadata capture and audit logging

The host also enforces the bridge between auth and storage. A managed asset can only become publicly deliverable when its capability state allows publication.

## Rendering API

Rendering is intentionally constrained. Extensions can contribute page fragments, admin widgets, metadata, translations, sitemap entries, JSON-LD nodes, and cache hints, but the host still owns template compilation, escaping, response composition, accessibility contracts, and HTTP cache semantics.

This keeps storefronts and admin surfaces consistent:

- extensions render into documented slots or fragment boundaries
- accessibility-aware component contracts still apply
- SEO primitives such as canonical tags and structured data remain typed
- the host decides whether a response is cacheable, personalized, or uncacheable

## Example

A customer-specific bookings extension might add an API endpoint that exposes waitlist availability. The handler can read event and reservation data through the events module's host API, call `check` before revealing restricted data, and emit a response with cache hints. If it also uploads a generated CSV for staff, the write goes through storage policy so the host can decide whether that file is private shared data, a signed download, or a local-only sensitive artifact.
