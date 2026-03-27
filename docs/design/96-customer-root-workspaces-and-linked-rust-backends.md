# Customer-Root Workspaces and Linked Rust Backends

**Part:** Customer Apps  
**Chapter:** 96  
**Status:** Accepted

This decision refines the customer-app architecture described earlier in the design set.

Davenda remains a reusable upstream platform made of core crates and official modules. Customer
stores remain separate deployable products. The accepted change is in how customer-owned native
behavior is integrated.

The platform will treat the customer project as the primary workspace root, with Davenda consumed
as a normal dependency rather than as the top-level repository that customer code is embedded into.
Customer-owned first-party logic will be linked into the build through a stable customer SDK and
explicit hook registration. Runtime-installed WASM extensions remain part of the design, but they
are the bounded third-party or marketplace customization path rather than the primary path for a
customer's own first-party business logic.

## Context

The platform design already draws a strong boundary between core, official modules, and customer
apps. That separation is one of the main reasons the rewrite exists. The earlier extension chapter
correctly emphasized that not all customization should become core or official native modules, but
it leaned too hard toward WASM as the default answer for customer-specific behavior.

That creates two problems.

First, a customer who owns the full application source should not be forced into the same runtime
boundary as an untrusted third-party plugin. A customer store is part of the product that will be
compiled, versioned, tested, and deployed by the team shipping that store. It should therefore be
able to participate in the build in a first-party way, through stable compile-time contracts rather
than only through sandboxed runtime hooks.

Second, treating customer-owned Rust logic as a separate sidecar service by default adds complexity
without solving the main customization problem. It introduces another process, another deployment
boundary, another transport contract, and another operational surface, even when the desired change
is simply "this customer needs one more checkout policy, admin workflow, or content rule." Sidecars
remain valid for integrations that genuinely need a separate process boundary, but they are not the
right primary model for normal customer-owned application logic.

The accepted architecture therefore needs three explicit customization tiers:

- Davenda core and official native modules for platform-owned first-party capabilities
- linked customer-owned Rust code for store-owned first-party behavior compiled into the product
- bounded WASM extensions for runtime-installed third-party or marketplace customization

## Decision

The accepted model is:

- the customer store is the workspace root
- Davenda is consumed as a normal upstream dependency, typically from crates.io or a pinned git ref
- the customer workspace may depend on `davenda-all` for the full official distribution or depend on
  individual Davenda crates directly when it wants a narrower battery set
- customer-owned Rust behavior is implemented in customer crates inside that workspace
- the customer binary links Davenda plus customer-owned crates together through a stable SDK and
  explicit hook registration
- third-party extensibility remains runtime-oriented and bounded through WASM extension contracts

This means the platform no longer treats "customer-specific behavior" and "runtime-installed plugin"
as the same thing.

## Why This Model Was Chosen

### It matches the product shape

Davenda is not supposed to be a giant application with hidden customer branches inside it. It is a
platform plus separate customer apps. Making the customer project the real workspace root is the
most honest way to reflect that in code.

### It avoids encouraging forks of core

Customers should not be pushed toward vendoring or modifying Davenda internals just because they
need custom product logic. A normal dependency relationship is healthier:

- Davenda stays an upstream product
- the customer pins versions explicitly
- upgrades remain visible and intentional
- unsupported source edits to Davenda become the exception rather than the recommended path

### It gives customer-owned code a first-party integration path

Customer code that ships as part of the store should have deeper and more ergonomic access than a
third-party runtime plugin. That does not mean direct access to unstable internal runtime details.
It means stable compile-time contracts, typed service facades, and explicit hook points.

### It keeps WASM focused on the right job

WASM remains valuable, but for a different reason. It is the right boundary for:

- runtime-installed extensions
- third-party marketplace packages
- bounded hooks and widgets
- integrations that must remain capability-limited and isolated from the native host

That is a much cleaner story than making WASM carry the entire burden of customer-owned
customization.

### It removes unnecessary sidecar complexity

A sidecar service is appropriate only when the work truly benefits from a separate process,
transport boundary, or lifecycle. It is not the right default answer for ordinary customer-specific
store behavior.

## Workspace and Packaging Model

A customer store should look roughly like this:

```text
harbor-shop/
  Cargo.toml
  Cargo.lock
  crates/
    harbor-shop-app/
    harbor-shop-backend/
    harbor-shop-bin/
  apps/
    harbor-shop/
      app.toml
      platform.toml
      templates/
      theme/
      auth/
      extensions/
```

In that layout:

- `harbor-shop-bin` is the executable composition root
- `harbor-shop-app` owns app composition helpers, install policy, and product-level config glue
- `harbor-shop-backend` owns customer-specific Rust logic and hook implementations
- `apps/harbor-shop/` still owns the customer manifest, templates, theme assets, auth package, and
  extension packages
- Davenda crates are normal dependencies declared in the customer workspace manifest

This model intentionally does **not** rely on dynamic Cargo manifest tricks, environment-variable
path interpolation, generated dependency hacks, or vendored copies of Davenda as the default
workflow.

## Capability Selection and Product Composition

Two separate choices need to remain separate:

- what code is available to compile
- what modules and behaviors are enabled for a particular app

Cargo dependencies answer the first question. The customer app manifest and platform config answer
the second.

That means:

- `davenda-all` is a convenience meta-crate that brings in the full official distribution
- direct dependencies on specific Davenda crates are the narrower composition path
- `app.toml` still decides which official modules are actually installed and active for the store
- the runtime validates that the app manifest only enables modules that the linked binary actually
  registered

This keeps packaging and product semantics separate while still giving customers fine-grained
control.

## Customer SDK and Hook Model

Davenda should expose a stable crate for customer-linked native integration, such as
`davenda-customer-sdk`.

That crate should provide:

- customer plugin registration traits
- stable hook traits for explicit lifecycle points
- typed facades over Davenda services
- stable request, response, and domain contract types
- strongly typed error contracts

It should **not** simply re-export unstable internals from runtime crates.

Example high-level shape:

```rust
pub trait CustomerBackendPlugin: Send + Sync + 'static {
    fn register(&self, registry: &mut dyn CustomerHookRegistry) -> Result<(), BackendError>;
}

pub trait CheckoutHooks {
    fn review_order(
        &self,
        ctx: &RequestContext,
        order: &OrderDraft,
        api: &dyn CommerceApi,
    ) -> Result<OrderReviewDecision, BackendError>;
}
```

The facades should expose supported first-party behavior such as:

- queueing a job
- reading or writing customer-owned data through stable repositories
- checking capabilities or explaining denials through supported auth contracts
- writing audit entries
- requesting outbound HTTP through policy-aware APIs
- publishing or inspecting managed assets through storage-aware APIs

The goal is to let customer code act like first-party product code **through stable boundaries**,
not to grant it arbitrary dependency on runtime internals.

## Binary Bootstrap Model

The customer binary becomes the composition root.

With the full official battery:

```rust
fn main() -> Result<(), anyhow::Error> {
    davenda_all::builder()
        .with_customer_plugin(harbor_shop_backend::plugin())
        .run_from_env()
}
```

With explicit subsystem selection:

```rust
fn main() -> Result<(), anyhow::Error> {
    davenda_runtime::Builder::new()
        .register_module(davenda_cms::module())
        .register_module(davenda_commerce::module())
        .register_module(davenda_memberships::module())
        .register_customer_plugin(harbor_shop_backend::plugin())
        .run_from_env()
}
```

This is the preferred product-composition model because it keeps the app's ownership legible:

- Davenda supplies the runtime and official modules
- the customer binary decides what to link and register
- the customer app manifest decides what to enable

## Role of WASM After This Decision

WASM remains a first-class part of the platform, but its role is now tighter and clearer.

WASM is the correct path for:

- third-party extensions
- runtime-installed packages
- marketplace distribution
- bounded widgets, hooks, webhooks, jobs, and branded fragments
- integrations that should remain capability-limited and isolated

WASM is **not** the default path for customer-owned first-party product logic when that logic ships
with the customer's own build and source tree.

## Role of Sidecars After This Decision

Sidecars are demoted from a primary customer-customization model to an optional integration pattern.

They are still appropriate when:

- a separate process boundary is operationally desirable
- the integration needs a distinct scaling profile or lifecycle
- the integration must be deployed independently
- a transport boundary is part of the real external-system contract

They are not the default answer for normal customer-owned Rust logic.

## Harbor Shop as the Reference Example

Harbor Shop should eventually demonstrate both supported customization paths:

- a linked customer-owned Rust backend crate implementing Davenda hook traits
- a bounded WASM extension installed through the normal extension packaging path

That split is intentional. Harbor Shop should show third-party developers and customer teams the
difference between:

- first-party customer code that participates in the build
- runtime-installed third-party code that participates only through explicit extension contracts

The existing sidecar-style example may remain temporarily as a migration aid, but it should not be
presented as the primary future model.

## Consequences

### Positive

- the customer-app boundary becomes clearer in code and packaging
- customer teams get a better native customization path
- Davenda remains an upstream dependency rather than encouraging silent source forks
- the separation between first-party customer logic and third-party runtime plugins becomes honest
- module selection becomes legible through normal dependency composition

### Negative

- Davenda must now own a stable customer SDK surface rather than relying on ad hoc internal access
- the bootstrap and registration model must become more explicit
- Harbor Shop and local development workflows will need to move from "embedded example app inside
  the Davenda repo" toward "reference customer workspace that consumes Davenda"
- some existing docs that describe WASM as the default customer-extension path need to be read in
  light of this decision and updated over time

## Guardrails

To keep this model healthy:

- customer crates must not be encouraged to depend directly on arbitrary runtime internals
- `davenda-customer-sdk` must stay smaller and more stable than the implementation crates behind it
- app-manifest module enablement must be validated against the modules linked into the binary
- Davenda upgrades must remain explicit through pinned versions rather than hidden vendored copies
- third-party runtime plugins must remain capability-scoped and isolated through WASM

## Relationship to Earlier Chapters

This decision refines the following areas:

- customer apps are still first-class deployable products
- official modules are still native first-party batteries
- WASM remains the bounded runtime extension system

What changes is the recommended default path for customer-owned native behavior:

- customer-owned first-party behavior is linked through a customer SDK and the customer workspace
- WASM is the preferred path for third-party runtime-installed plugins
- sidecars are optional, not primary

This decision should therefore be treated as an accepted clarification of the customer-app model,
not as a rejection of the broader core/module/customer split.
