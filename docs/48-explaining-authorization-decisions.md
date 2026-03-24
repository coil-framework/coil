# Explaining Authorization Decisions

**Part:** Authorization and Security  
**Chapter:** 48

Authorization is too central to remain a black box. The platform therefore treats explanation as part of the auth product, not as a debugging afterthought. A denied action must be traceable to the capability that was checked, the model version that was active, and the relation path that matched or failed to match.

## Explain API
Core provides an explanation surface alongside the normal `check`, `list`, `lookup`, and `expand` APIs. An explanation should identify:

- the actor, resource, and capability under evaluation
- the model package and version used for the decision
- the capability binding that was resolved
- the tuple chain or derived-permission expansion that granted access, or the point where evaluation failed
- any recursion limits, cycle protection, or scope constraints that affected the result

This is the data developers need when an editor cannot publish a page, a support operator cannot refund an order, or event staff cannot check in a booking they expected to manage.

## Where Explanations Are Available
Explanation is intentionally not a public API for end users. It belongs in developer tooling, CLI diagnostics, and privileged admin support surfaces. WASM extensions may call `check` if granted, but `explain` is reserved for dev and admin contexts because it can reveal sensitive relationship structure. Logs and audit trails should record enough information to correlate the final decision with the request or job that triggered it without dumping raw internals into customer-facing error pages.

## Operational Use
The real benefit of explanation is supportability. Suppose a media manager attempts to publish an asset and is denied. The support view should be able to show that `asset.publish` resolved against the current model, that the actor has `asset.read` through one folder relationship, but lacks the publishing relation required by the active policy. That turns the incident from guesswork into a precise policy problem.

Strong authorization without explanation produces brittle systems and angry operators. Strong authorization with explanation produces a platform that can be extended, audited, and debugged without bypassing the very controls it is trying to enforce.
