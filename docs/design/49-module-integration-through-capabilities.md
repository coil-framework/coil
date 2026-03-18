# Module Integration Through Capabilities

**Part:** Authorization and Security  
**Chapter:** 49

Capabilities are the stable boundary between authorization policy and module code. A module declares the actions it needs and asks core to evaluate them. It never bakes relation names, tenant hierarchies, or fixed roles into its own implementation.

## Capability Contracts
Capability names should be explicit and domain-scoped, following a pattern like `domain.resource.action`. The examples already used across the platform set the tone:

- `cms.page.read`
- `cms.page.publish`
- `catalog.product.edit`
- `admin.users.manage`
- `asset.publish`

These names are not cosmetic labels. They are versioned contracts consumed by first-party modules, customer-app code, and, where allowed, WASM extensions. If a module needs a new semantic that would widen or materially alter behavior, it should introduce a new capability rather than silently reinterpret an old one.

## Integration Rules
Every official module should ship a capability manifest that lists:

- required capabilities for its core actions
- optional capabilities for extra features
- the resource kinds those capabilities apply to
- any extension slots where third-party code may participate

At startup or install time, core validates that the active auth model binds the required capabilities. If it does not, the module is not correctly configured. That failure must happen before serving traffic.

Module code then uses only the host auth API. A CMS page publish action asks for `cms.page.publish` on a page reference. A booking operation asks for the booking capability that its manifest declares. The engine resolves the capability through the current model package. That is what keeps modules portable across default and custom auth models.

## Consequences for Module Authors
Capability integration is more disciplined than sprinkling role checks through handlers, but it is also what makes the platform's promised modularity work. Modules become explicit about what they need, customer apps can replace auth semantics safely, and explanation tooling can tell operators exactly which contract failed. Without that layer, "replaceable auth model" would collapse the first time a first-party module assumed its favorite relation name.
