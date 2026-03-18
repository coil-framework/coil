# Request, Job, and Event Lifecycles

The platform has three main execution paths: HTTP requests, background jobs, and domain events. They share infrastructure, but they are not the same thing and should not be modeled as interchangeable callbacks. One of the reasons the new platform exists is to avoid the hidden lifecycle coupling that accumulates in systems built around broad hook registration.

## Request Lifecycle

An HTTP request enters through direct TLS termination or through a trusted reverse proxy. Transport middleware normalizes forwarded headers, request identity, and scheme before the platform resolves the hostname to a customer app and site configuration. Routing and middleware then derive locale, region, session state, authenticated principal, CSRF policy, cache variation context, and trace metadata.

Once the request reaches a handler, business logic runs through domain services rather than through templates or middleware side effects. The handler may call authorization checks, load domain data, consult cache layers, and choose a response shape, but the work should remain explicit. Rendering may produce a full page, an HTML fragment for progressive enhancement, a redirect, a typed API response, or an asset delivery response. Response middleware finalizes cookies, cache headers, diagnostics, and any deferred side effects.

## Job Lifecycle

Background jobs carry work that should not block user-facing latency or should be retried independently. Typical examples include email dispatch, payment follow-ups, image processing, storage synchronization, import and export flows, report generation, webhook retries, and search or sitemap maintenance. Jobs must accept explicit input payloads, run with traced and observable execution context, and remain idempotent so that retries do not corrupt state.

Jobs are not fake requests. They should not depend on ambient browser context, hidden session state, or ad hoc globals. If a job needs a principal concept for audit or policy reasons, that principal should be carried explicitly in the job payload or reconstructed through stable services.

## Domain Event Lifecycle

Domain events are typed signals emitted by meaningful state transitions inside the system. They exist to coordinate modules and background workflows without recreating WordPress-style hook soup. Examples include order completion, membership activation, booking cancellation, page publication, or managed asset publication.

An event may be handled synchronously inside a transaction boundary when the reaction is part of the same consistency requirement, or it may be turned into queued work when the reaction is slow, fan-out heavy, or operationally isolated. What matters is that event schemas are deliberate and owned, not arbitrary strings emitted by anyone.

The relationship between the three lifecycles is straightforward. A request may enqueue jobs and emit domain events. A job may emit new domain events as it completes work. Event handlers may enqueue more jobs. What should never happen is the loss of boundaries: jobs should not pretend to be web requests, events should not become unrestricted interception points, and internal code should not bypass auth, tracing, or transaction rules simply because it is "inside the platform."
