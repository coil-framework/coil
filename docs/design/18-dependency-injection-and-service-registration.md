# Dependency Injection and Service Registration

The platform needs a composition model that is explicit enough for Rust and still practical for modules and customer apps. "Dependency injection" in this context does not mean a dynamic container that can manufacture anything on demand. It means a clear composition root, stable service contracts, and predictable registration of the pieces that make up a running application.

Startup should assemble a service graph from core, the selected official modules, and the customer app. Core registers the foundational services: configuration, logging, tracing, database access, migrations, cache backends, storage drivers, asset manifest services, TLS services, the template runtime, the authorization engine, and the WASM host runtime. Those services form the stable substrate that all higher-level code depends on.

Official modules then register their own domain services, route groups, background handlers, capability requirements, metadata providers, admin resources, and storage or cache policy participants. The important point is that module registration happens against stable contracts. A module should be able to request "cache with tagging support" or "capability checker" without knowing how the customer app chose to implement every surrounding detail.

Customer apps participate by selecting modules, supplying configuration, binding capabilities to the chosen auth model, registering theme and template packages, and optionally providing customer-specific adapters or extension bundles. This is where app-specific composition belongs. The customer app should not need to rewire core, but it should be able to select how the assembled platform is shaped for one deployment.

WASM extensions sit outside the normal service graph. They do not resolve arbitrary native services through the DI system. Instead, they use host APIs exposed by the runtime, and those APIs are constrained by capabilities, resource limits, and versioned contracts. This keeps the extension boundary honest and prevents the service container from becoming a second informal plugin system.

The runtime should therefore have one obvious composition root where service wiring is visible, testable, and debuggable. That makes startup failures easier to understand, simplifies integration testing, and reduces the temptation to solve design problems with hidden global lookups. In a platform intended to replace WordPress-style ambient power, explicit composition is not optional plumbing. It is one of the main architectural safeguards.
