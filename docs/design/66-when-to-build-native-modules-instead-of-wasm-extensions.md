# When to Build Native Modules Instead of WASM Extensions

**Part:** Extensibility  
**Chapter:** 66

The platform's default customization story is WASM, but the default product story is not. Core is never WASM, and the main first-party batteries are native first. The practical rule is simple: use WASM for bounded customization at the edge, and use native modules when the feature becomes part of the product's structural spine.

## Build a Native Module When the Platform Must Trust It Deeply

Native modules are the right fit when a feature needs durable ownership of shared behavior or state. That usually includes:

- major domain packages such as CMS, catalog, checkout, admin, memberships, or events
- features that own tables, migrations, and long-lived business data
- flows that need deep transaction participation
- high-volume or latency-sensitive paths where sandbox overhead and ABI constraints are unwelcome
- functionality that must integrate tightly with auth, caching, storage, SEO, or rendering internals
- features the platform team intends to support across many customer apps for years

This is why the official batteries are native first. Rebuilding them inside the sandbox would force the platform to optimize around the lowest-common-denominator ABI before the core contracts are mature.

## Use WASM When the Work Is Specific, Bounded, and Replaceable

WASM extensions are the better fit for:

- customer-specific pages and endpoints
- admin widgets
- pricing or promotion rules that vary per customer
- integration adapters and webhook consumers
- jobs and workflows that orchestrate existing host APIs
- search, indexing, or reporting add-ons that are intentionally optional

These are valuable, but they should remain detachable. The sandbox gives enough power to customize the product without letting every custom feature become hidden platform internals.

## Decision Criteria

If a feature matches several of these conditions, it should probably be native:

- it defines a new shared data model consumed by multiple official modules
- it requires unrestricted access to storage, secrets, or outbound networking
- it needs low-level rendering control rather than fragment contribution
- it must participate in migrations, auth capability bindings, and cache invalidation as a first-class platform citizen
- it is performance critical enough that auth batching, query plans, and render behavior need native tuning

If the feature is mostly orchestration around existing contracts, WASM is usually still correct.

## Graduation Path

Some features will start as extensions and later deserve promotion. That is healthy. A customer-specific workflow can begin in WASM, prove value, and then move into a native official module when it becomes reusable, operationally critical, or too constrained by the sandbox.

The key is to keep the boundary honest. Do not force native batteries into WASM for symmetry, and do not leave deeply trusted product code in WASM just because it started there. The platform remains maintainable only if the trusted, performance-sensitive, widely reused layers stay native.
