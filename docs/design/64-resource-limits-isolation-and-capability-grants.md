# Resource Limits, Isolation, and Capability Grants

**Part:** Extensibility  
**Chapter:** 64

The extension model is only credible if the sandbox has teeth. WASM code is allowed to customize the product, but it is not allowed to consume unbounded resources, pierce customer isolation, or silently acquire broader powers than the customer app intended to grant.

## Isolation Model

Isolation is defined around extension point and customer app. A page handler, webhook consumer, admin widget, and scheduled job may all be implemented by the same package, but they run as separate registered handlers with separate capability checks. The host may reuse runtime instances internally, yet the security model is per invocation, not per process.

Every invocation is namespaced by:

- customer app
- site, brand, or locale context where relevant
- installed extension identity and version
- declared extension point

That prevents one customer's customization from becoming ambient state for another customer or another execution surface.

## Capability Grants

Capability-based permissions are the control plane for extensions. The package manifest declares what the extension wants. The customer app installation decides what is actually granted. The runtime then exposes only those granted handles at execution time.

Typical grants cover:

- resource reads and writes through module-owned APIs
- auth checks and, in rare cases, tuple mutation
- storage reads and writes under approved policy classes
- outbound HTTP to approved integrations
- named secrets required for those integrations
- render slots, admin surfaces, or webhook subscriptions

A denied capability should fail closed and produce an observable audit event. Extensions must never infer access from route placement or from the presence of data in their input payload.

## Resource Limits

The core runtime should enforce limits for:

- execution time
- memory usage
- outbound HTTP behavior
- storage volume and object size where relevant
- concurrency and queue pressure for background workloads

The exact numeric limits can vary by surface, but the contract is stable: request handlers and admin widgets should be short-lived, jobs may run longer but still remain bounded, and webhooks should never block the delivery system indefinitely.

## Operational Behavior When Limits Are Hit

When an extension exceeds its envelope, the host should terminate the work, mark the invocation as failed, and emit enough diagnostics to explain why. The important point is that the failure belongs to the extension invocation, not to the whole application process.

Operators need to see:

- which extension and version failed
- which limit triggered
- which customer app and extension point were affected
- whether the failure is retriable

## Design Consequence

This model is why high-trust platform code belongs in native modules instead of the sandbox. If a feature needs open-ended database control, deep transaction participation, long-running compute, or unrestricted secrets and network access, it has already crossed the line where a native module is the better fit.
