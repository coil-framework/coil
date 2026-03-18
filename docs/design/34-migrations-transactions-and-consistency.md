# Migrations, Transactions, and Consistency

**Part:** Data and Storage  
**Chapter:** 34

Schema evolution is a platform concern because the runtime is assembled from core, official native modules, and customer-app code. The migration system therefore needs clear ownership and ordering rules. Core owns its own schema. Each native module owns migrations for the tables and indexes it introduces. Customer apps may add migrations for customer-specific models, but they do not modify module-owned schema casually. Installation and upgrade tooling then composes those streams into a single ordered plan.

The operational default is additive and online-safe change. A module upgrade should prefer forward-compatible schema changes, staged rollouts, and background backfills where needed instead of brittle big-bang migrations. That matters because the platform expects multi-node deployments, distributed caches, object storage, and long-lived customer apps. A migration strategy that assumes the whole system can pause and restart in lockstep will fail quickly.

Transactions are equally opinionated. Critical write paths must be explicit about their boundaries, especially in bookings, reservations, payments, subscription updates, and auth-tuple changes. The platform should encourage idempotent command handling and should make it easy to bundle related database writes into a single transaction. Where external side effects are involved, such as payment gateways or outbound notifications, the model should prefer durable state transitions plus queued follow-up work rather than pretending those systems participate in a single database commit.

Consistency also matters across reads performed inside a transaction. Auth decisions that rely on the Zanzibar-style tuple engine need a clear snapshot policy when invoked from transactional workflows. Recursive CTE evaluation, cache invalidation, and module-owned domain updates must all agree on which committed state is visible at each point. The framework does not need full distributed transactions, but it does need a coherent story for "what did we check and when?"

WASM extensions participate through host-defined units of work. They may request domain actions or contribute logic inside a bounded operation, but they should not hold open raw database transactions or perform side effects that the host cannot coordinate. This keeps the consistency model intelligible and prevents extension code from becoming the place where invariants go to die.

The practical test for this chapter is simple. If a booking is created, a reservation is consumed, the relevant capacity views are invalidated, and follow-up notifications are enqueued, the platform must be able to explain exactly which pieces are transactional, which are asynchronous, and how retries remain safe.
