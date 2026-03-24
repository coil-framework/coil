# The Data Layer and Query Model

**Part:** Data and Storage  
**Chapter:** 33

Core ships with a real data layer because the platform needs more than ad hoc SQL helpers. It needs migrations, transactions, validation support, pagination, and a disciplined query model that can be shared by official native modules and customer apps. The point is not to force one fashionable pattern everywhere; it is to prevent domain logic from dissolving into controllers, templates, and extension code.

The data layer separates reads from writes conceptually, even when both use the same storage engine. Queries are expressed through typed filters, pagination structures, and module-owned repositories or query services. Mutations go through domain services with explicit transaction boundaries and idempotency requirements where the business domain demands them. This is especially important for bookings, subscriptions, payments, and publication workflows, where "just update a few rows" is not a safe abstraction.

Module ownership is the second core rule. Each official module owns its schema, query services, and invariants. Customer apps may compose those modules and add their own data models, but they should not reach directly into module tables to assemble private shortcuts in the frontend or admin layer. If a CMS page needs event data or a booking screen needs membership state, the composition should happen through stable module APIs or shared domain services, not through ungoverned cross-module joins hidden in templates.

The query model is also where auth, locale, and caching begin to intersect with persistence. Public resource queries need publishability and locale awareness. Personalized queries need subject context for capability checks and cache scoping. High-frequency reads should be compatible with the platform cache stack without making correctness depend on stale assumptions. None of those concerns belong in raw template logic, which is why the platform insists on typed query and presenter boundaries.

WASM extensions do not receive direct database access. They call host APIs that expose approved query surfaces or invoke domain actions. That is partly a security boundary and partly an architectural one: the platform wants extension logic to remain portable across internal refactors of the native data layer.

In practice, a paginated event listing, a customer order history, and an admin resource table should all feel consistent from the outside. They may be backed by different modules and policies, but they should use the same ideas: explicit query inputs, explicit pagination, explicit authorization, and no invisible business logic in the view layer.
