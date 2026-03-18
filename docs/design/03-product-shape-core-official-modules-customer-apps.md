# Product Shape: Core, Official Modules, Customer Apps

The platform is intentionally organized into three layers, and most architectural decisions become clearer once that split is taken seriously. The goal is not to build one giant application with optional features. The goal is to build a reusable host runtime, a set of first-party batteries, and separate deployable customer apps that compose them.

## Core

Core is the native host framework. It owns the HTTP runtime, routing, middleware, request and response types, configuration, service wiring, migrations, caching, queueing, scheduling, storage abstractions, observability, and the WASM host runtime. Core also owns the platform-wide services that must behave consistently no matter which modules a customer app installs: authorization, capability resolution, internationalization primitives, SEO primitives, accessibility contracts, TLS lifecycle support, HTTP cache semantics, and asset publication rules.

Core is not a storefront, a CMS, or a default admin product. It provides the execution model and the extension contracts that higher-level packages depend on. That distinction is important because it keeps the runtime lean and prevents every product feature from becoming a hard dependency of every customer app.

## Official Modules

Official modules are the first-party batteries. They implement reusable product domains such as CMS, admin, catalog, checkout, memberships, subscriptions, events, bookings, media, search, reporting, and other common capabilities. These modules are native, not sandboxed, because they need deep access to core services such as transactions, rendering, authorization, storage, cache invalidation, and background workflows.

Official modules must still be modular. A customer app may install the full commerce and admin stack, only the CMS and media pieces, or a combination that includes custom domain modules for one vertical. That is why official modules consume capability contracts rather than assuming one fixed auth schema, and why they use core services rather than inventing private versions of caching, storage, or i18n.

## Customer Apps

A customer app is the deployable product for a specific customer. It selects which official modules to include, provides hostnames and environment policy, chooses locale behavior, binds capabilities to the chosen authorization model, supplies templates and theme assets, and adds customer-specific extensions or frontend behavior. In practice, this is where the real product identity lives.

Customer apps are separate applications, not just tenant records inside a master install. Core still provides tenant, site, brand, and storefront primitives because they are useful for many customers, but the platform is not primarily a giant multi-tenant control plane with thin skins on top. The main product model is shared framework plus separate customer implementations.

For the first customer replacement, the app shape is broader than generic ecommerce. The app combines commerce, memberships, subscriptions, events, timeslots, bookings, branded CMS concerns, and a customer-specific admin experience. That validates why the platform split matters: the customer app can compose several official modules without forcing all of that logic into core, and without turning its own code into an unsupported fork.

The three-layer model can be summarized simply:

- core provides the stable runtime and cross-cutting contracts
- official modules provide reusable product batteries
- customer apps provide composition, presentation, policy, and customer-specific behavior

Everything else in the platform should fit somewhere in that structure. If a feature does not, its ownership is probably not clear enough yet.
