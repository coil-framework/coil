# Long-Term Governance of the Platform

**Part:** Migration and Evolution  
**Chapter:** 87

The platform is intended to outlive any single customer implementation. That only happens if it is run as a product with clear ownership, explicit admission criteria, and disciplined release policy. Without governance, a multi-customer framework quickly turns into an internal dumping ground of one-off features, half-supported extension points, and undocumented compatibility breaks.

## Ownership Model

Governance starts with named stewardship for each layer:

- core maintainers own runtime, auth, storage, cache, TLS, templating, host APIs, and cross-cutting contracts
- module maintainers own official CMS, commerce, memberships, events, media, and admin packages
- customer app teams own templates, content models, configuration, custom auth bindings, and customer-specific extensions
- security ownership is shared but must have explicit authority over auth, secrets handling, certificate lifecycle, and extension sandbox policy

Ownership is not just about who writes code. It determines who can approve breaking changes, who answers incidents, and who is responsible for documentation accuracy.

## Criteria For Core

Core should remain small and strict. A capability belongs in core only if it is:

- required by nearly every install
- difficult or dangerous to bolt on later
- foundational to module or extension behavior

That is why auth, cache, storage, TLS, i18n, SEO primitives, and WASM hosting belong in core, while catalog, checkout, memberships, events, and admin applications do not. Putting customer-shaped product features into core recreates the bloat this platform is intended to avoid.

## Criteria For Official Modules

Official modules exist for repeatable product concerns across customers. A module is a good candidate for first-party status when:

- multiple customer apps need the same domain capability
- the capability depends on deep integration with core contracts
- the platform team is willing to support its migrations, tests, documentation, and versioning over time

An official module is not just code. It is a support commitment.

## RFC And Change Control

Changes to core contracts, the auth engine, host APIs, config schema, storage policy model, or capability registry should go through an RFC-style process. The RFC should state:

- the problem being solved
- what layer owns the change
- compatibility impact
- migration and rollback implications
- extension and customer-app consequences

Small fixes do not need ceremony, but architectural drift usually begins with “just this once” changes to shared surfaces. The RFC process is how the platform protects itself from those shortcuts.

## Documentation And Quality Gates

Documentation is part of the definition of done for anything that changes a published platform surface. At minimum, maintainers should update:

- narrative docs for design intent
- reference docs for schemas, capabilities, or CLI behavior
- migration notes where behavior changes
- test coverage for the affected layer

The platform should also keep reference installations healthy. If the commerce example or the events-and-memberships example stops upgrading cleanly, governance has already failed before any customer notices.

## Support Policy

The platform should publish a clear support window for:

- active major lines of core
- supported module versions within those major lines
- WASM host ABI versions
- auth model package formats and migration tooling

Security fixes should flow quickly to supported lines. Unsupported versions should be explicitly marked as such in release notes and tooling. Quiet abandonment is not governance.

## Protecting The Boundary

The most important long-term rule is that customer-specific behavior belongs in customer apps unless it has been deliberately promoted. Promotion to an official module should require evidence that the behavior is genuinely reusable and that the platform team is prepared to support it. If that bar is not met, the correct home is a customer app or a customer-owned extension.

Strong governance is what keeps a reusable platform reusable.
