# Observability and Runtime Diagnostics

**Part:** Operations  
**Chapter:** 72

Observability has to cross the whole stack: native core, official modules, customer-app configuration, and WASM extensions. The platform is intended to run commerce, CMS, memberships, and events workloads with customer-specific behavior on top. That only stays debuggable if every layer emits telemetry in the same language and with the same correlation model.

## Standard Signals

Core should emit structured logs, metrics, and traces for every important request and background path. The minimum shared dimensions are:

- customer app
- site or brand where relevant
- route or extension point
- module or extension identity
- outcome, latency, and error classification

This lets an operator answer basic production questions without reconstructing context from multiple systems by hand.

## Important Runtime Diagnostics

The platform should make the following areas visible by default:

- request and fragment render latency
- cache hit rates and invalidation activity
- auth check volume, batching effectiveness, and decision latency
- queue depth, retry counts, and dead-letter growth
- webhook verification failures and replay rejections
- object-store sync backlog and signed-delivery errors
- TLS issuance and renewal status
- extension timeouts, capability denials, and sandbox traps

For a personalized platform, auth and cache behavior matter as much as raw request timing. A slow capability check or missing auth batching can be the real cause of bad page performance.

## Explainability

Some diagnostics need richer explain APIs instead of just counters. Two examples are central:

- auth explain, so operators and developers can see which tuple chain or capability binding granted or denied access
- extension diagnostics, so a failed WASM invocation can be tied to a specific package, version, extension point, and limit breach

Explainability should be available in developer or admin contexts, not as a public endpoint.

## Environment-Specific Behavior

Development and staging should expose more aggressive diagnostics such as N+1 detection, noisy query traces, and template-fragment timing. Production should keep the same model but sample or redact as needed to protect latency and sensitive data.

## Customer-Specific Incidents

Because customer apps can install different module combinations, theme layers, auth models, and extensions, dashboards and alerts must stay partitionable by app. A webhook backlog in one customer app should be visible without looking like a platform-wide outage. The same applies to certificate issues, storage policy failures, and broken custom widgets.

The practical goal is simple: when something breaks, operators should be able to identify whether the fault belongs to core, an official module, a customer-app configuration choice, or a specific extension within minutes rather than hours.
