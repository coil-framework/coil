# Testing Strategy, Fixtures, and Quality Gates

**Part:** Core Runtime  
**Chapter:** 22

Testing exists to protect the platform boundaries that make this architecture viable: the split between core and official modules, the capability contract between modules and the auth engine, the HTML-first rendering model, the storage policy model, and the WASM host boundary. A rewrite like this fails when those seams drift, so the test strategy is built around them.

## Test Layers Follow the Architecture

Core maintains the lowest-level tests: pure unit tests for parsing, rendering, policy evaluation, cache key construction, locale resolution, and other deterministic logic; integration tests against real Postgres for migrations, transactions, and recursive CTE authorization; and subsystem tests for storage, cache adapters, and TLS behavior. These tests are allowed to be implementation-aware because core owns those implementations.

Official modules are tested at the contract level. A module must prove that it registers its routes correctly, respects capability checks instead of hard-coding relation names, renders valid full-document and fragment responses, emits the right metadata and JSON-LD, declares correct cache invalidation, and honors storage policy instead of reaching around it. If a module cannot pass in a minimal reference app, it is not truly modular.

Customer apps own composition tests. Their job is to prove that installed modules, theme overrides, locale policy, auth bindings, and customer-specific workflows still form a coherent product. A customer app does not retest the entire framework, but it does test the business journeys that matter to that customer: account sign-in, membership purchase, event discovery, booking, admin moderation, media upload, and page publication.

WASM extensions sit under a separate contract harness. They are tested against host APIs, declared capabilities, and resource limits. The important rule is that extension tests verify behavior through the same stable boundary used in production. An extension is never considered correct because it happened to work while linked to an internal database crate.

## Fixtures Model Real Installations

Fixtures are installable scenarios, not piles of SQL inserts. Every scenario should describe a coherent world: installed modules, auth model package, locales, themes, media library state, site or brand configuration, and domain data such as products, memberships, events, reservations, or bookings. That makes fixtures usable for development, integration tests, and release rehearsals.

The platform needs deterministic fixture builders for values that otherwise make tests flaky: clocks, generated ids, slug generation, locale fallback, signed URLs, and asset publication manifests. A scenario that publishes an event page must always produce the same canonical URL, the same JSON-LD payload shape, and the same auth decision chain for the same subject and resource.

The most important fixtures are cross-cutting ones. A "published event with booking enabled" fixture should include the event record, timeslot capacity, booking rules, translated copy, media assets, SEO metadata, relevant auth tuples, and cache tags. Those are the fixtures that catch integration mistakes between modules long before production does.

## Rendering, Accessibility, and Search Are Testable Contracts

Because the platform is HTML-first, rendered output is a primary artifact. Full documents and fragments should be tested semantically rather than as arbitrary strings. Tests assert landmarks, headings, forms, canonical links, hreflang tags, structured-data nodes, and stable fragment ids or data hooks. Small markup changes are acceptable; breaking the document contract is not.

Accessibility and SEO are therefore part of the quality gate, not post-release review. First-party UI must pass automated accessibility checks and route-level keyboard or focus smoke tests. Search-facing routes must validate canonical behavior, sitemap inclusion, robots directives, and typed JSON-LD output. Official modules that emit products, events, articles, or organization pages are responsible for shipping those assertions with the module itself.

## Data, Auth, and Storage Need Real Integration Coverage

The auth engine runs on Postgres recursive CTEs, so allow or deny behavior has to be exercised against a real database. The same applies to migrations, transaction boundaries, and the change-propagation rules that affect caches and jobs. For storage, the platform tests against local filesystem backends and S3-compatible backends, because sync state, signed delivery, and metadata handling are part of the contract.

The test suite should also prove the negative cases that are easy to miss in a modular system: a private asset is not accidentally published through CDN delivery, a locale-specific page is not served from the wrong cache key, an extension without storage permission cannot write arbitrary objects, and a capability rename in an auth model package does not silently break an official module.

## Release Gates

Core is releasable only when unit, integration, migration, and contract suites pass across the supported backends and the reference app still boots cleanly. Official modules are releasable only when they pass their own contract suites plus integration runs against the current core release candidate. Customer apps are releasable only when their scenario suites, migration rehearsal, asset publication checks, and smoke tests pass for the exact module set and auth model they intend to ship.

This is stricter than a framework with a single application in mind, but it has to be. The platform is promising that core, official batteries, and customer apps can evolve independently without turning into another opaque monolith. The test strategy is how that promise stays real.
