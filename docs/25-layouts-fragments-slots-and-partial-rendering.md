# Layouts, Fragments, Slots, and Partial Rendering

**Part:** Rendering and Frontend  
**Chapter:** 25

The composition model for UI has to serve two masters at once: it must let official modules ship reusable screens and fragments, and it must let customer apps impose their own brand, navigation, and page structure without forking those modules. Layouts, fragments, and slots are the mechanism that makes that practical.

## Documents Are Built From Explicit Pieces

A full page render is assembled from a layout plus named fragments. The layout defines the outer document structure: `<html>`, `<head>`, global navigation, flash messaging, footers, and the major content regions that every page in a given shell should share. Fragments provide reusable pieces such as hero sections, cards, filters, booking panels, admin tables, or metadata blocks. Slots let a higher-level template inject content into well-defined placeholders without requiring the callee to know the caller's entire structure.

The important constraint is that composition is explicit. A fragment declares the model it needs and the slots it exposes. It does not reach into ambient page state and hope the right globals happen to exist. That keeps fragments portable between official modules and customer apps and makes them safe to use as standalone partial responses later.

## Override Order Mirrors the Product Shape

Core provides the composition rules and any global layout helpers. Official modules provide their own default layouts where necessary, but most commonly they provide fragments and screen templates that assume a host shell. Customer apps sit on top and choose the actual site or admin shell, route-level layout selection, theme assets, and brand-specific fragment overrides. In practice that means a customer app can keep the events module's booking fragment while wrapping it in a completely different storefront shell and navigation system.

This is also the right place to enforce modularity. A customer app can replace a fragment or provide content for a slot, but it should not have to reimplement the module's business semantics to change the look and feel. If changing the theme requires copying an entire admin screen, the module boundary is wrong.

## Partial Rendering Is the Same Composition Model, Not a Second One

Fragment endpoints render the same named fragments used inside full pages. They do not use a parallel mini-template system. When an inline table refreshes, a booking panel updates, or a media browser paginates, the server is rendering a fragment with the same input model, accessibility semantics, cache rules, and localization behavior that the full document path would have used.

That uniformity matters operationally. It means partial responses can be tested with the same render contract, cached with the same scope rules, and debugged with the same template diagnostics. It also prevents the common failure mode where full pages are one UI system and "AJAX partials" are another that slowly diverges.

## Slots as Stable Integration Points

Slots are how official modules remain brandable and extensible without becoming unstructured hook soup. A storefront layout may expose slots for page title adornments, breadcrumb trails, aside content, and footer promotions. An admin shell may expose slots for action bars, filter summaries, or widget regions. Customer apps and, where appropriate, WASM extensions can fill those slots through declared contracts.

The key is that the slot is a stable interface, not an invitation to rewrite the module. The host controls where slot content can appear, how many times it can render, what data it receives, and which accessibility rules apply. That keeps extension power compatible with predictable documents.

## Example: Event Detail and Booking Panel

The first reference customer app is a good example. The customer app owns the event page shell, site header, brand tokens, and content framing. The events module owns the event facts, timeslot summaries, booking CTA fragment, and any check-in or waitlist controls. The booking panel is rendered as a named fragment. On initial page load it sits inside the full event document. During interaction it can be re-rendered by a fragment endpoint as the user changes date or availability, but it remains the same fragment with the same semantics.

That is the intended payoff of the composition model: a reusable module surface that still supports strongly branded customer apps and high-quality partial rendering without duplication.
