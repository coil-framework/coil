# Customer-Specific Auth and Capability Mappings

**Part:** Customer Apps  
**Chapter:** 79

Authorization is core platform architecture, but the customer app decides how that architecture is bound for a real product. The platform ships a default auth model package, yet each customer app may extend it or replace it entirely. The non-negotiable rule is that official modules depend on capabilities, not on fixed relation names from the default model.

## The Three Layers

Customer-app auth work is easiest to reason about when the pieces stay separate:

- tuple storage and execution engine in core
- auth model package that defines resource types, relations, and derived permissions
- capability bindings that connect module needs to the chosen model

These pieces version independently. That keeps "replaceable schema" from collapsing into one vague concept.

## Extend Mode

Most customer apps will start by importing the default model and then extending it. Typical changes include:

- adding resource types for customer-specific content or operations
- adding new relations for local organizational structure
- introducing derived permissions that map back to existing module capabilities
- binding extra asset or media permissions such as `asset.publish` or `asset.manage_storage`

This keeps the common platform concepts intact while still allowing a customer-specific organization model.

## Replace Mode

Full replacement is valid when the customer's authorization model is materially different. In that mode, the app disables the default model and supplies:

- its own resource and relation definitions
- its own migrations and bootstrap rules
- explicit capability bindings for every official module or extension it uses

If those bindings are incomplete, the installation should be considered invalid. Replacement only works when capability coverage is explicit.

## Testing and Explainability

Before a customer app goes live, auth bindings should be tested through the same stable APIs the runtime uses:

- `check`
- `list`
- `lookup`
- explain in developer or admin contexts

This is especially important when content models, asset publication rules, and multi-brand admin roles differ from the default assumptions. A capability contract is only credible if the app can prove how it resolves.

## Practical Boundary

Customer apps own the model choice and bindings. Core owns execution, batching, caching hooks, and explain tooling. Official modules remain portable because they ask for capabilities. WASM extensions remain portable because they also ask for capabilities through host APIs. That is the mechanism that lets the platform support strong auth without freezing every customer into one org model.
