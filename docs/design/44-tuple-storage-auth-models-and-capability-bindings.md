# Tuple Storage, Auth Models, and Capability Bindings

**Part:** Authorization and Security  
**Chapter:** 44

The platform keeps three authorization concepts separate because mixing them makes the system brittle. Tuple storage is the raw fact store. The auth model defines what those facts mean. Capability bindings connect that meaning to stable module contracts. Each layer changes at a different rate and belongs to a different part of the platform.

## Tuple Storage
Tuple storage is core infrastructure. It records relationship facts such as who belongs to a team, which group administers a site, or which actor may manage an asset folder. The storage schema is engine-owned and optimized for graph traversal, batching, invalidation, and versioning. Official modules and extensions do not treat the tuple tables as their private persistence layer. They ask core to read or write tuples through the supported API, and only when the runtime has granted that authority.

## Auth Models
An auth model is a declarative package that defines resource types, relations, derived permissions, and validation rules. It also carries the migrations and bootstrap data required to evolve that model over time. The shipped default model covers common platform concepts, but the point of the system is that a customer app may extend it or replace it entirely. Because the model is packaged separately from the engine, the framework can upgrade query behavior and explanation tooling without forcing everyone into the same org chart.

## Capability Bindings
Capability bindings are the contract between auth and application code. A module does not ask whether someone is `page#editor`; it asks whether the actor has `cms.page.publish` on a page resource. The active model package binds that capability to whatever graph logic is appropriate for the deployment. A default installation might satisfy `asset.publish` through `site.admin` and `asset_folder.editor` relationships. A customer-specific model might require a separate compliance group. The media module does not change in either case.

This separation is what lets the platform have strong defaults without fake modularity. Core guarantees the tuple engine and evaluation APIs. Model packages define authorization semantics. Modules depend only on capabilities. When those boundaries hold, customer apps can change policy without rewriting first-party code.
