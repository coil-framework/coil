# CMS and Content Management Modules

**Part:** Native Batteries  
**Chapter:** 53

The CMS layer is an official module family, not part of core, because content modeling is product behavior. Core provides the rendering, i18n, SEO, media, storage, and auth primitives. The CMS modules turn those primitives into editorial workflows.

## Scope
The first-party CMS distribution covers the common needs of content-heavy customer apps:

- page and content-type definitions
- navigation structures
- redirects
- drafts, previews, revisions, and scheduled publishing
- media references
- forms and form submissions where they belong to the content experience

Customer apps still own the actual content schema and presentation decisions. The CMS modules provide the machinery; the app decides what content types exist, which templates render them, and how brand-specific structure is expressed.

## Cross-Cutting Contracts
CMS modules are required to consume the platform-level services rather than inventing their own editorial side systems. Localized fields, localized slugs, and per-locale SEO metadata sit on the core i18n and SEO primitives. Publishing invalidates the right cache keys, sitemaps, navigation fragments, and JSON-LD output through core cache and metadata hooks. Media references go through the managed asset system, so publication of a page and publication of the underlying asset remain coherent.

Authorization is capability-driven. The CMS code checks capabilities such as `cms.page.read` and `cms.page.publish`; the active auth model decides who can satisfy them.

## Editorial Workflow
A typical page moves through draft, previewable unpublished state, scheduled publication, and live publication. That workflow is not just a content concern. It affects routing, cache invalidation, sitemap inclusion, canonical metadata, and media eligibility. The CMS modules therefore coordinate with the admin shell, auth engine, and media library rather than treating publishing as a single boolean on a row.

This is a good example of the overall platform split: core makes localized rendering, typed metadata, and cache invalidation possible; the CMS modules turn those capabilities into an editor-facing product.
