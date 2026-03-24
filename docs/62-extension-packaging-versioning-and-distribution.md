# Extension Packaging, Versioning, and Distribution

**Part:** Extensibility  
**Chapter:** 62

WASM extensions are distributed as versioned application add-ons, not as ad hoc code drops. A customer app pins the exact extension artifacts it trusts, and the host activates only packages whose manifest, capability requests, and ABI compatibility match the running platform.

## Package Shape

An extension package contains at least three things:

- the compiled WASM artifact
- a manifest that declares identity, version, target ABI, extension points, and required capabilities
- an app-facing configuration schema so installation is explicit and validated

The manifest is the durable contract. It describes what the extension wants to do, not how it was implemented internally. That lets the host reject packages that request undeclared surfaces or that target an incompatible runtime contract.

In practice the manifest should declare:

- package name and publisher
- semantic version of the extension
- supported host ABI or contract range
- registered handlers for pages, APIs, jobs, webhooks, widgets, or render hooks
- required capabilities for auth, data access, storage, outbound HTTP, or secrets
- extension-specific configuration keys and defaults

## Versioning Rules

There are three distinct version lines and they must not be conflated:

- core version, which owns the host runtime and ABI
- official module versions, which remain native and are installed separately from core
- extension version, which tracks the package's own behavior and configuration

Compatibility is determined first by the ABI range declared in the extension manifest. The host refuses activation when that range does not match. That keeps upgrades explicit and avoids WordPress-style surprises where arbitrary code executes against changed internals.

Capability changes are treated as compatibility events. If a new package version asks for broader access than the currently installed version, the customer app must approve that change during installation or upgrade.

## Distribution Model

Official batteries are not shipped in this package format by default. Commerce, CMS, admin, memberships, and events are native first-party modules, versioned separately from core and selected per customer app. WASM packaging exists for the customization layer: customer-specific code, third-party extensions, and narrowly scoped first-party add-ons that deliberately fit the sandbox.

Customer apps can consume extensions from:

- a private internal registry
- a local artifact checked into the app repository
- a curated first-party catalog of supported add-ons

The host should verify integrity before activation. Official distribution should publish signed or checksum-pinned artifacts, and private registries should expose equivalent integrity metadata. The key requirement is deterministic trust, not a specific registry product.

## Upgrade and Rollout Policy

Extensions are installed and upgraded per customer app. The safe default is pinning, not floating latest. A rollout should validate:

- ABI compatibility with the current platform
- compatibility with the installed official modules the extension consumes
- configuration schema changes
- capability deltas
- any storage namespaces or persistent state the extension expects

This keeps extension churn from becoming hidden platform drift. If an extension becomes broadly reused, needs deeper transaction or rendering control, or starts owning important shared data, it should usually graduate into a native official module rather than accumulate permanent complexity at the WASM boundary.
