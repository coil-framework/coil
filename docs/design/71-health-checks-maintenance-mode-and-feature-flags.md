# Health Checks, Maintenance Mode, and Feature Flags

**Part:** Operations  
**Chapter:** 71

These controls are part of the platform contract, not optional conveniences layered on later. The runtime, official modules, and customer apps all depend on predictable health signals, a safe maintenance workflow, and controlled rollout mechanisms. Without them, a modular platform quickly becomes hard to operate in production.

## Health Checks

Health checks should be split by purpose rather than exposed as one vague status endpoint.

Liveness answers whether the process should be restarted. It should stay cheap and avoid deep dependency checks.

Readiness answers whether the node is currently fit to receive traffic. It should reflect the state of the dependencies that must be healthy for request handling in that role, such as:

- database connectivity and migration compatibility
- distributed cache reachability where required
- queue connectivity for nodes that also accept webhook or async work
- extension registry and configuration load success
- object-store or secrets access where the node cannot serve correctly without it

Separate synthetic or operator-focused checks can exercise deeper storefront and admin flows without being used as load balancer probes.

## Maintenance Mode

Maintenance mode is a deliberate operational state, not a deployment accident. It should be possible to enable it:

- globally for a deployment
- per customer app
- in read-only form for mutating routes only

During maintenance, the platform should serve a controlled response for affected traffic, allow operators to bypass where necessary, and continue only the background work explicitly deemed safe. The point is to stop user-facing churn while preserving a predictable operational envelope.

## Feature Flags

Feature flags exist to support staged rollout, canaries, and customer-specific enablement. They should be scoped and observable. Useful targeting dimensions include:

- customer app
- site or brand
- environment
- operator-controlled rollout cohorts

Flags are not a substitute for authorization or configuration modeling. A flag can turn on a feature; it should not become the permanent system of record for who may perform a protected action.

## Interaction With Extensions and Modules

Official modules and WASM extensions may read flag state and health-related context through approved contracts, but the control plane remains in core. A customer app can use flags to introduce a new CMS flow or extension-backed integration gradually, while readiness and maintenance behavior still remain consistent across the deployment.

## Operational Rule

If a feature cannot explain:

- how it affects readiness
- what happens during maintenance
- how it is rolled out safely

then it is not yet production-ready on this platform.
