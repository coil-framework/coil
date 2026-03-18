# Theme System and Design Tokens

**Part:** Rendering and Frontend  
**Chapter:** 27

Customer apps exist so that each installation can look and feel like its own product without forking the framework. The theme system is how that customization happens while preserving the boundaries between core, official modules, and customer-specific code.

## Themes Belong to Customer Apps

Core does not own brand expression. It owns the rules by which themes are discovered, resolved, and applied. Official modules ship semantic markup, template fragments, and default styling hooks that are intentionally generic enough to survive across customers. The customer app then chooses the actual shell layouts, visual language, asset bundles, and token values that turn those modules into a particular storefront, membership site, or admin surface.

That split is important because the platform is not a single multi-tenant website with skins bolted on. It is a reusable framework plus separate customer apps. A theme therefore lives with the customer app and can vary by site, brand, or hostname where the customer's own product requires it.

## Design Tokens Are Platform Data, Not Ad Hoc CSS Variables

The platform should define a formal token vocabulary for colors, typography, spacing, sizing, radii, borders, elevation, motion, density, breakpoints, and iconography. Tokens are part of configuration and rendering, not just a Sass convenience. Templates and official modules refer to semantic tokens such as surface, accent, success, muted text, compact spacing, or form radius. The customer app maps those semantics to actual brand values.

Treating tokens as platform data has two advantages. First, official modules can remain visually adaptable without hard-coding brand assumptions. Second, the system can lint tokens for accessibility and consistency. A theme that drops contrast below the platform baseline, disables visible focus, or ignores reduced-motion preferences is not a harmless styling choice; it is a contract violation.

## Override Model

Theme customization happens through declared surfaces. Customer apps can provide layout templates, replace approved fragments, contribute CSS and JavaScript bundles through the asset pipeline, and set token values per site or brand. Official modules should expose enough semantic hooks and slots to make this practical, but not so much that every module becomes a collection of undocumented DOM internals.

The platform should therefore document which parts of a module are structural and which are thematic. For example, the admin shell can promise action bars, filter regions, tables, pagination controls, and dialog surfaces. The customer app can restyle and rearrange within those contracts, but the underlying semantics remain stable so accessibility, progressive enhancement, and module upgrades continue to work.

## White-Label and Multi-Brand Support

White-label requirements are already part of the target workloads, so theme resolution cannot assume one global brand per deployment. Core should resolve theme selection from the same site or brand primitives used elsewhere in the platform. A customer app can then map hostnames, sites, or brands to token sets, layouts, navigation structures, and asset bundles without cloning business logic.

This becomes especially important in the first reference customer app, where commerce, memberships, events, and CMS content need to coexist under branded experiences. The events module should not know whether a particular customer uses one brand, several regional brands, or different storefront and admin themes. It should render semantic fragments that the customer app can style and frame appropriately.

## Theme Governance

Themes are part of the shipped product, so they need the same rigor as other modules. Token validation, template override linting, asset manifest checks, and accessibility smoke tests belong in the customer app's quality gates. A theme is successful when it can dramatically change the product's presentation while leaving module contracts, rendering guarantees, and upgrade paths intact.
