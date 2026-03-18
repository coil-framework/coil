# Why the Platform Uses WASM

**Part:** Extensibility  
**Chapter:** 60

The platform uses WASM because it wants controlled extensibility, not because it wants to implement the whole system inside a plugin sandbox. The design rule from the chat is clear: native spine, WASM edge.

## What WASM Is For
WASM is the default customization layer for customer-specific behavior that benefits from isolation and versioned host contracts. Good candidates include:

- custom pages and endpoints
- admin widgets
- pricing or promotion rules
- webhook handlers
- jobs and workflow steps
- search or indexing adapters
- customer-specific business logic that should not become a first-party native module

This gives the platform a stable plugin ABI, explicit extension registration, capability-based permissions, and resource limits around time, memory, outbound HTTP, storage, and secrets access.

## What WASM Is Not For
Core is never WASM. Major first-party modules are not forced into WASM either. The framework proper must retain native control of transactions, auth evaluation, rendering, debugging, performance-critical paths, storage credentials, and other trusted runtime concerns. Forcing core or the real batteries into the sandbox would collapse the architecture into the lowest common denominator too early.

Official modules may still expose extension slots and selectively dogfood the same boundary that third parties use. That is useful because it proves the contracts are real. It does not mean the checkout engine, auth runtime, or admin shell should themselves be implemented as sandbox payloads.

## Why This Boundary Matters
WASM gives the product a safer customization story than a global hook system. A customer app can add custom logic for an event workflow or a catalog integration without gaining unrestricted access to the runtime. The host stays responsible for authorization, storage policy, HTTP cache semantics, and secret handling. Extensions participate through declared contracts, not ambient privilege.

That is the real reason the platform uses WASM: not to make everything pluggable, but to make customization possible without giving up the integrity of the native core.
