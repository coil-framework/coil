# Versioning Policy Across Core, Modules, and Apps

**Part:** Migration and Evolution  
**Chapter:** 86

Versioning is a governance tool, not a packaging detail. The platform has multiple replaceable layers, and they only remain replaceable if each layer advertises its own compatibility contract. Core, official modules, customer apps, auth model packages, capability registries, and WASM extensions all version independently because they solve different problems and break for different reasons.

## Core Policy

Core uses semantic versioning around published host behavior:

- HTTP and middleware contracts
- template and rendering APIs
- cache, storage, TLS, i18n, SEO, and observability primitives
- auth engine APIs and tuple-storage behavior
- WASM host APIs and extension lifecycle contracts

Within a major line, core may add new optional features but must not break existing documented contracts. Any planned removal is first deprecated, documented, and surfaced in tooling before the next major release.

## Official Module Policy

Official modules version separately from core. A module release declares:

- its own semantic version
- the range of supported core versions
- the capability registry versions it expects
- any auth model package requirements or new capability bindings
- any schema migration requirements

This matters because a customer may need a new CMS feature without taking a new events module, or a new events module without changing its customer-specific frontend. Separate module versioning keeps those decisions possible.

## Customer App Policy

The customer app is its own deployable artifact with its own version number. Its version should reflect changes to:

- templates and theme assets
- installed module set
- customer-owned configuration defaults
- translations and locale policy
- auth model extensions or replacements
- customer-specific native code or WASM extensions

The customer app does not redefine the compatibility policy of core or official modules. Instead, it declares the exact versions it is built and tested against.

## Auth And Capability Versioning

Authorization has three distinct versioned surfaces:

- tuple or storage schema
- authorization model
- capability bindings

They should never be collapsed into one generic “auth schema version.” A capability rename may be a breaking change for official modules without changing tuple storage. A storage migration may be required for performance without altering capability semantics. A customer app may replace the default authorization model while preserving the same capability contract. Treating them separately is what makes the replaceable auth model real.

## Extension Compatibility

WASM extensions target host APIs and declared capability contracts. Each extension package should declare:

- supported WASM host ABI versions
- required host APIs
- required capability names and minimum capability registry versions
- any module-specific extension points it consumes

If an extension depends on an unofficial internal host function, it is outside policy and may break at any time.

## Configuration Compatibility

Configuration is also a versioned surface. New fields may be added in minor releases, but incompatible meaning changes belong to a major release. Deprecated keys should remain readable for one major line, with CLI warnings and automated migration help where possible. Customer content or editorial data should not be moved into config just to avoid a migration. Config remains for runtime policy; content remains in managed data.

## Supported Upgrade Paths

The supported path is always explicit:

- patch to patch within a compatible line
- minor to minor within a major line
- major to major through documented migration steps

Skipping across unsupported version combinations is not guaranteed, even if a package manager could technically resolve it. The release process therefore publishes a tested compatibility matrix and recommended upgrade bundles.

## Practical Rule

Official modules depend on capabilities, customer apps depend on contracts, and extensions depend on host APIs. Nobody should depend on internal relation names, private tables, or ad hoc module internals. That rule is more important than the exact version numbering scheme, because it is what allows versioning policy to work at all.
