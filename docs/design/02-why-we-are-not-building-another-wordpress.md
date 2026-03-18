# Why We Are Not Building Another WordPress

The fastest way to fail this rewrite would be to preserve the WordPress mental model while changing the implementation language. Recreating plugins, themes, hooks, and global registration inside a new runtime would keep the same coupling problems, only with different syntax. The platform would still blur the line between framework, CMS, product modules, and customer customization, and it would still struggle to explain why any given behavior exists.

WordPress is built around broad mutability. Code can attach itself almost anywhere, execution order is often implicit, and extensions are usually trusted with the same ambient power as the host system. That flexibility is acceptable for a large general-purpose ecosystem, but it is the wrong foundation for a platform expected to run bookings, payments, memberships, media, and admin operations across multiple customers. Those workloads need stronger guarantees than "some callback may have changed this on the way through."

The new system deliberately separates responsibilities that WordPress tends to collapse together. Core owns the runtime, the contract surface, and the non-negotiable cross-cutting services. Official modules provide reusable product batteries such as CMS, admin, commerce, memberships, events, and media. Customer apps compose those pieces into an actual website or product experience for a given customer. That split is the opposite of the WordPress model, where the CMS is the center of gravity and everything else hangs off it.

This is also why the platform does not treat third-party extensibility as equivalent to first-party module implementation. Core is never implemented as WASM. Official batteries are native first-party modules, versioned separately from core and installed into customer apps as needed. WASM exists for controlled customization at explicit extension points such as pages, API endpoints, pricing rules, admin widgets, webhooks, or background jobs. The platform must be extensible, but it must not grant every extension the same access and privilege as the native runtime.

Several anti-goals follow from that position:

- no global hook soup as the main integration mechanism
- no requirement that first-party batteries live inside the same sandbox as third parties
- no assumption that every customer wants the same admin surface or the same module set
- no template language that turns HTML views into a general scripting environment
- no plugin path that silently bypasses authorization, storage policy, cache rules, or observability
- no architecture in which CMS concerns implicitly dictate the runtime model for every other feature

Avoiding "another WordPress" does not mean rejecting all of WordPress's useful lessons. The existing system proves that content tooling, media handling, page composition, admin workflows, and extension points matter. The mistake would be to inherit the old packaging of those ideas. In the target platform, those concerns become explicit modules and contracts instead of ambient framework behavior. That is what allows the system to stay lean without becoming inflexible.
