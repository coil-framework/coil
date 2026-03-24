# Boundaries Between Core, Native Modules, WASM, and Customer Apps

The platform only remains maintainable if each layer has a clear authority boundary. Most architecture failures in systems of this kind come from blurry ownership: business rules drifting into templates, customer code reaching into framework internals, or extensions quietly becoming alternative runtimes. This chapter defines the intended boundaries so that modularity does not turn into ambiguity.

## Core Boundary

Core owns the execution model. That includes the HTTP runtime, middleware pipeline, request and response primitives, configuration loading, service registration, database access primitives, migrations, caching contracts, storage contracts, observability, TLS lifecycle support, the authorization engine, and the WASM host runtime. Core is allowed to expose stable interfaces, but it should not depend on assumptions that belong to one vertical, one admin workflow, or one customer.

Several concerns belong in core even when they are visible to product teams. Internationalization, locale routing, SEO metadata primitives, accessibility contracts, cache backends, asset publication rules, and object-storage policy are cross-cutting enough that the platform needs one definition of them. Allowing modules to reinvent those services would create incompatible behaviors across the same installation.

## Native Module Boundary

Official modules are native first-party packages that integrate deeply with core through stable contracts. They can register routes, migrations, domain services, admin resources, background handlers, sitemap emitters, capability requirements, and template fragments. They are expected to participate fully in transactions, cache invalidation, tracing, and authorization. That is why they are not forced through the same sandbox used for third-party code.

Native modules are still constrained by platform rules. A commerce module does not define its own auth engine. A CMS module does not invent a private storage abstraction. An events module does not bypass the cache model or emit opaque side effects outside the job and event infrastructure. Modules gain deep integration, not license to behave like mini-frameworks.

## WASM Boundary

WASM is the controlled extension surface. It exists so customer-specific and third-party logic can contribute meaningful behavior without receiving unrestricted native access. The host should expose explicit APIs for the use cases the platform wants to support: routes, fragments, API handlers, jobs, webhooks, metadata providers, pricing and promotion logic, admin widgets, and similar extension points.

That boundary must stay narrow. WASM extensions should call host functions for permission checks, rendering, storage requests, cache hints, enqueueing jobs, or emitting metadata. They should not read auth tuples directly, hold raw object-store credentials, own certificate issuance, or bypass observability. Resource limits on time, memory, outbound HTTP, storage access, and secret access are part of the contract, not optional hardening after the fact.

## Customer App Boundary

A customer app is the composition boundary. It selects official modules, supplies templates and theme assets, chooses locales, binds capabilities to its chosen auth model, configures storage and certificate strategy, and adds customer-specific native adapters or WASM extensions. The customer app is allowed to shape the product, but it is not expected to replace framework services such as routing, caching, TLS lifecycle, or the authorization engine.

This also clarifies the practical placement test:

- if the behavior is required by every installation, it belongs in core
- if it is reusable product behavior for many customers, it belongs in an official module
- if it is customer-specific and can live behind an explicit host contract, it belongs in the customer app or a WASM extension

The platform should defend these boundaries aggressively. The whole point of the design is to avoid rebuilding WordPress-style power through less honest names.
