# Frontend Integration Boundaries

Frontend code succeeds on this platform only when its boundary with the backend is explicit. The rendering model is server-first, but that still leaves several important integration points: templates need data, enhancement code needs stable hooks, fragment requests need routes and response contracts, and customer apps need room to add branded behavior without silently rewriting module semantics.

## The Server Owns Document Shape and Business State

The basic flow is route to handler to view model to template to document or fragment response. That is the authoritative path. A page is not assembled by letting client code fetch a set of unrelated APIs and guess how they fit together. The handler chooses the data to expose, the template chooses the semantic HTML, and the server emits the metadata, cache headers, and auth-gated controls that belong with that response.

That makes the DOM a real integration contract. Customer-side code may enhance a rendered form, tab set, filter panel, or media browser, but it starts from server-owned markup. The platform should therefore prefer stable data attributes, fragment identifiers, form names, and semantic component boundaries over undocumented CSS-selector conventions.

## Two Approved Client-to-Server Paths

The first path is fragment-oriented interaction. Client code requests another HTML fragment from a normal route and swaps it into the document. This is the preferred path for most rich behavior because it preserves the rendering, auth, locale, accessibility, and cache guarantees of the server.

The second path is an explicit JSON or machine API. Those APIs are versioned, typed, and treated as public contracts for integrations or genuinely client-heavy features. They are not informal backchannels for skipping the normal rendering layer. If an account page, event browser, or admin table can be expressed as fragment updates, it should not also invent a second private API surface without strong reason.

## Responsibilities by Layer

Core owns route registration, response types, fragment mechanics, asset manifest lookup, CSRF and session integration, and the host APIs that any extension must use. Official modules own their handlers, view models, templates, fragment ids, and any first-party enhancement controllers that are part of the module's shipped UI. Customer apps own theme bundles, optional customer-specific scripts, and approved template overrides.

WASM extensions are constrained on purpose. They can register routes, fragments, widgets, and APIs through the extension system, but they still go through host auth, cache, storage, and rendering services. They do not get raw access to internal module stores, nor do they inject arbitrary client bundles into the application shell at runtime. The goal is extensibility without giving up operational control.

## Example: Admin Table With Progressive Filters

An admin bookings table demonstrates the boundary well. The server renders the initial table, filters, pagination controls, and any action buttons the current subject is authorized to use. A customer app may add richer filter affordances or keyboard shortcuts through its own bundle. When the filters change, the browser requests the updated table fragment and summary counts from the server. If an external reporting integration needs machine-readable exports, that is a separate, versioned API route.

This is the discipline that keeps the frontend from turning into a shadow architecture. Every integration point is real, but each one is constrained enough to remain debuggable and evolvable.
