# Domain Modeling Across Modules

**Part:** Data and Storage  
**Chapter:** 35

The platform is intentionally modular, which means it cannot collapse into a single shared schema with vague ownership. Domain modeling starts with a simple rule: every native module owns its concepts, tables, invariants, and public integration surface. Core owns platform-wide primitives such as sites, brands, locales, capabilities, storage policies, and other shared runtime concerns. Customer apps own their own models and composition logic on top of that. Everything else should have a named owner.

This rule is especially important because the default batteries are not narrow. Commerce, CMS, memberships, subscriptions, events, bookings, admin tooling, and media all need to cooperate. Cooperation, however, is not the same as flattening. An events module may reference a site, use shared auth capabilities, and react to membership state, but it still owns event and timeslot concepts. A commerce module may depend on media and localization primitives, but it still owns catalog and ordering concepts.

Cross-module references should therefore be explicit and stable. Shared identifiers, domain events, capability checks, and well-defined service interfaces are the preferred integration points. Hidden cross-module joins and direct writes into another module's tables are the failure mode to avoid. They create tight coupling and make independent module evolution impossible, which is exactly the kind of bloat the platform is trying to avoid.

The auth model reinforces this separation. Official modules should depend on capabilities such as publishing, editing, managing storage, or refunding, not on hard-coded assumptions about another module's private role names or tuple relations. That same principle applies to domain behavior: a module should ask whether an action is allowed or whether a stable service contract can provide information, not whether it can reinterpret another module's internals.

Customer apps sit above this as composition layers. The first reference customer app combines commerce, memberships, events, bookings, and branded CMS behavior into one product, but it still does so by selecting modules and configuring how they fit together. A different customer app may choose catalog and CMS without events. Another may replace the default auth model while still satisfying the same capability contracts. The domain model has to survive those permutations.

The result is a platform that feels integrated without pretending that every feature belongs to one giant application schema. That is the architectural line between reusable batteries and a new form of WordPress sprawl.
