# The WASM Runtime and Host Boundary

**Part:** Extensibility  
**Chapter:** 61

The platform uses WASM as its default customization layer, not as the implementation substrate for core or major first-party batteries. Core remains native Rust, official modules are native first, and WASM is the boundary where customer-specific or third-party code is allowed to participate. This is the architectural rule behind the phrase "native spine, WASM edge."

## What Runs Inside the Runtime

WASM extensions are intended for bounded pieces of customization:

- custom pages and route handlers
- API endpoints
- webhook consumers
- jobs and scheduled workflows
- admin widgets and data providers
- pricing, promotion, and search adapters
- render hooks and page fragments

Each extension is registered explicitly. There is no global hook soup and no implied execution based on file presence. The host loads only the handlers declared in the extension manifest and only at the extension points that the customer app has installed.

## What the Host Continues to Own

The host owns the parts of the system that must stay coherent across every customer app:

- HTTP runtime, routing, middleware, and request lifecycle
- transactions, migrations, and direct data-store access
- the auth engine, capability system, and auth model bindings
- cache engines, queue infrastructure, scheduler, and storage backends
- TLS, certificate management, and edge integration
- template compilation, fragment composition, and response finalization
- metrics, tracing, structured logs, and operational controls

An extension does not receive raw database connections, raw object-store credentials, or direct access to auth tables. It receives a versioned host contract and works through host APIs. That keeps authorization auditable, storage policy enforceable, and runtime behavior observable.

## Boundary Contract

The WASM boundary is a contract, not a source-level coupling. Extensions exchange typed payloads with the host through a stable ABI. The exact in-memory layout is a host concern; extension authors program against generated bindings and documented request and response types.

Every boundary crossing is versioned and capability checked. The host passes:

- invocation context, including customer-app identity, locale, and site or brand context
- extension configuration for the installed app
- capability-scoped handles for data, auth, storage, rendering, and outbound HTTP where granted
- tracing and diagnostic context so logs and spans stay correlated with the parent request or job

The extension returns intent, not infrastructure ownership. For example, it can return a page fragment, JSON response, webhook result, cache hint, or storage write request, but the host still decides how the final response is emitted, cached, or persisted.

## Invocation Model

Isolation is defined per invocation surface. Request handlers, jobs, webhook consumers, admin widgets, and data providers are treated as separate execution contexts with their own capability grants and resource limits. The host may pool runtimes for efficiency, but that pooling is invisible to the extension and never changes the capability envelope for a call.

The expected lifecycle is:

1. Resolve the registered extension and handler for the current extension point.
2. Materialize the invocation context from the native request, job, or event.
3. Attach only the capabilities granted to that extension in this customer app.
4. Execute the handler inside the WASM sandbox with time and memory limits.
5. Validate the returned response shape before merging it back into the host pipeline.

## Practical Example

An events customer app might install a WASM extension that adds a waitlist-admin widget and a webhook-driven enrichment job. The widget can ask the host whether the current admin user has `events.waitlist.manage`, read the event and reservation data exposed by the events module, and render an approved admin fragment. The webhook job can process a verified external payload and enqueue follow-up work. In both cases, the extension stays inside the host's auth, storage, and observability model rather than bypassing it.
