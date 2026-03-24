# Progressive Enhancement and Interactive Patterns

**Part:** Rendering and Frontend  
**Chapter:** 26

The platform's interactive model starts from a simple rule: every important user journey must remain correct as plain HTTP with server-rendered HTML, standard forms, redirects, and links. JavaScript improves the experience, but it does not become the only path through the system.

## Baseline Behavior Comes First

Forms post to normal handlers. Successful writes use redirect-after-post semantics. Validation errors re-render the same document or fragment with accessible feedback. This baseline matters for reliability as much as for accessibility. If the booking flow, account management, or admin publishing workflow only works when a particular bundle hydrates correctly, the platform has already broken its own operating model.

Because baseline behavior is server-owned, business state also stays server-owned. Reservation holds, checkout state, publication transitions, and auth-sensitive actions are all committed by the backend. Client code may display progress, collapse sections, or make the interaction faster, but it does not become the source of truth for whether something is booked, published, or authorized.

## Preferred Enhancement Pattern: HTML Fragments

The default enhancement mechanism is a fragment request that returns more HTML. That can be driven by htmx-style conventions, small first-party controllers, or similarly lightweight behavior, but the transport is the important part. Filters, pagination, inline validation, booking availability panels, notification preference toggles, media pickers, and admin table updates are all natural fits for fragment-based enhancement.

This keeps UI composition aligned with the rendering model from the previous chapters. The server returns markup that already contains the right locale, accessibility semantics, auth-gated controls, cache scope, and metadata hooks. The browser swaps or appends it into the document. No client-side view framework needs to reconstruct what the server already knows.

## JSON and Rich Client State Are Explicit Exceptions

Some use cases still warrant JSON: third-party integrations, public or partner APIs, search suggestions backed by a dedicated client widget, or genuinely complex local interactions such as drag-heavy editing tools. Those cases are allowed, but they must be chosen deliberately. The platform does not turn every button click into an API design exercise by default.

The boundary is simple. Ephemeral presentation state can live in the browser. Authoritative business state stays on the server. If the UI needs to know whether a user has permission to publish an asset, whether a timeslot still has capacity, or whether a locale-specific page is visible, it asks the server through a normal handler or fragment route.

## Accessibility, History, and Failure Handling

Progressive enhancement is only credible if partial updates remain accessible. Fragments must preserve heading structure, move focus intentionally, and announce meaningful changes where appropriate. Enhanced forms still need full validation summaries and field-level error wiring. Interactive controls must degrade to ordinary links or forms when scripting is absent or fails.

The platform should also treat browser history, URL state, and replay safety as first-class concerns. Filters and pagination that matter to navigation should remain URL-addressable. Mutating actions must preserve CSRF protection and idempotency guarantees. Background work such as image processing or asset sync should not be disguised as front-end interactivity when it really belongs in jobs.

## Ownership Across Layers

Core owns the primitives: fragment responses, form handling, redirect helpers, validation rendering, client-asset registration, and any first-party enhancement runtime. Official modules define the interactive patterns appropriate for their domains. Customer apps decide how far to enhance those patterns visually and where to introduce bespoke behavior. WASM extensions may add fragment endpoints, widgets, and workflow handlers through host APIs, but they remain bound to the same CSRF, auth, cache, and rendering contracts as native code.

That keeps the platform from collapsing into a split-brain system. Interactivity is real, but it is still part of a server-rendered application.
