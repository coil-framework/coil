# CLI, Dev Tooling, and Local Development

**Part:** Core Runtime  
**Chapter:** 21

The platform CLI is the operational face of the framework for developers. It is not a grab bag of helper scripts. It is the stable way to create customer apps, install official modules, validate auth models, run migrations, publish assets, inspect cache state, and debug the composed system that will eventually ship to production.

## The CLI Is a Core Contract

Core owns the root command surface because only core can see the full installation: runtime configuration, registered modules, the active customer app, storage backends, auth capability bindings, and extension registrations. Official modules add namespaced commands for the domain they own, such as CMS content import, media repair, event reindexing, or commerce catalog maintenance. Customer apps may add project-specific commands, but they do so through the same registration model rather than by bypassing the framework with bespoke shell scripts.

The stable command families follow the platform boundaries:

- workspace and scaffolding for new customer apps, native modules, and WASM extensions
- environment bootstrapping for databases, caches, local object storage, TLS, and seeded reference data
- database and auth commands for migrations, tuple-model validation, capability inspection, and explain tooling
- runtime commands for the web server, worker pool, scheduler, and background repair tasks
- asset and storage commands for manifest generation, object-store publication, sync reconciliation, and media inspection
- diagnostics for routes, dependency registration, cache namespaces, template resolution, and module health

The important architectural decision is that the CLI operates on declared platform concepts. A developer asks the system to "publish assets for the current customer app" or "explain why this subject cannot publish this asset," not to run a crate-specific script with hidden assumptions.

## Local Development Mirrors Production Primitives

Local development must preserve the same core invariants as production even when the topology is simplified. The reference local stack therefore includes Postgres, Redis or Valkey, an S3-compatible object store, the HTTP server, background workers, and the scheduler. If a customer app depends on locale routing, auth capability checks, asset publication, or object-store-backed uploads in production, local development must exercise those same paths rather than replacing them with in-memory fakes.

Core supports two local modes. The fast path runs the application binary directly with attached local services and template or asset watching enabled. The full path boots the same service graph that production uses, including workers and a local reverse-proxy or TLS endpoint, so developers can debug cache headers, signed media delivery, scheduler leadership, and multi-process behavior. Both modes read the same typed configuration model. The difference is operational topology, not application semantics.

Local certificates are part of this contract. The platform's TLS subsystem already knows how to terminate TLS, select certificates, and route hostnames to sites or brands. Local development uses the same boundary, but certificates come from a development CA or generated local trust root rather than from ACME or Cloudflare Origin CA. That keeps customer apps honest about hostname, redirect, cookie, and secure-origin behavior without forcing production certificate issuance into the development loop.

## Scaffolding and Composition

Scaffolding is opinionated because the platform itself is opinionated. Creating a new customer app should produce a workspace member with theme directories, template roots, translation catalogs, token files, app configuration, module installation declarations, fixture hooks, and a baseline test suite. Creating an official module should generate its migration package, capability binding declaration, template namespace, assets, localization files, and contract tests. Creating a WASM extension should generate the guest entrypoints, manifest, capability requests, and host-API test harness.

This matters because the platform is explicitly split into core, official modules, and customer apps. The CLI is how that split stays legible over time. A customer app should never need to copy a large amount of code from the reference app just to add a theme, install the events module, or replace the default auth model package. Likewise, a module author should not have to reverse-engineer how to register routes, migrations, templates, and asset bundles.

## Hot Reload, Fixtures, and Developer Feedback

The shortest feedback loop comes from the pieces that are safe to reload independently. Templates, translation catalogs, token files, and content fixtures can reload without recompiling the whole binary. Rust code, native modules, and capability binding logic still rebuild normally, but the CLI is responsible for stitching those changes back into a coherent local runtime and for surfacing failures in terms of platform concepts rather than compiler noise alone.

Fixture loading is part of local development, not just testing. A developer should be able to boot the reference customer app with sample brands, locales, media assets, auth tuples, memberships, events, and bookings so the runtime can be explored as a real system. Because the target workloads are commerce, memberships, events, and branded CMS experiences, a nearly empty database is not a useful development environment.

## Diagnostics and Failure Modes

The platform depends on invisible infrastructure such as authorization, cache scoping, fragment selection, and storage policy resolution. The CLI therefore exposes explain-style diagnostics as first-class features. Developers need to inspect route resolution, template override order, rendered asset manifests, cache tags for a page, storage policy chosen for an upload, current scheduler leadership, and the tuple chain that granted or denied a capability.

These commands are also the primary defense against WordPress-style drift. When a customer app behaves unexpectedly, the answer should come from a consistent diagnostic surface instead of from searching through hidden hook registrations. Local tooling is successful only if it makes the composed system understandable, not merely runnable.
