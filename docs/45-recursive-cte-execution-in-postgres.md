# Recursive CTE Execution in Postgres

**Part:** Authorization and Security  
**Chapter:** 45

The authorization engine evaluates relationship graphs in Postgres using recursive CTEs. That is a deliberate choice, not an incidental implementation detail. It keeps policy resolution close to the transactional data model, lets auth participate in the same operational story as the rest of the platform, and avoids splitting security-critical graph evaluation into a separate infrastructure tier before the product even stabilizes.

## Execution Strategy
A capability check starts with three pieces of data: actor, capability, and resource. Core resolves the capability binding for the active auth model, then expands the relevant relations through recursive SQL until it finds a satisfying path or exhausts the search space. `list`, `lookup`, and `expand` queries use the same model, but they change the direction of traversal and the shape of the returned set.

Because this is a general graph engine, the runtime must own the operational guardrails:

- cycle detection so bad model definitions cannot recurse forever
- bounded recursion depth and query-shape limits
- index design tuned for common subject, relation, and object lookups
- model and namespace versioning
- snapshot semantics for checks performed inside transactions
- batched APIs so lists, admin tables, and bulk jobs do not degenerate into N+1 authorization queries

Precomputed or materialized paths are intentionally not the default. They are an optimization to introduce only when measured workloads prove the recursive path is insufficient.

## Explainability and Caching
Recursive CTE execution is only acceptable if operators can reason about it. The engine therefore keeps enough structure around a decision to explain which capability binding was evaluated, which relation expansion matched, and where a denial occurred. That same structure feeds caching and invalidation. Cached auth answers are safe only when tuple changes, model changes, and resource version changes can invalidate them predictably.

## Example
Consider an admin table showing all bookings a staff member may check in. The platform should not issue one authorization query per row. Instead, the booking module calls a batched lookup against the `booking.check_in` capability. Postgres evaluates the relationship graph set-wise, core returns the authorized booking identifiers, and the admin shell renders only the allowed rows. That is the intended performance model: graph-aware SQL, batched APIs, and no hidden permission logic in module code.
