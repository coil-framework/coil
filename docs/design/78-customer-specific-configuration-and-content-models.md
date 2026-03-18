# Customer-Specific Configuration and Content Models

**Part:** Customer Apps  
**Chapter:** 78

Configuration and content models are the sanctioned place for customer variability. They let a customer app adapt site behavior, locale policy, schemas, and editorial structures without turning every difference into custom code. This is one of the main tools for keeping the platform upgradeable.

## Configuration Scope

Customer-app configuration covers the things the platform expects to vary per implementation, including:

- domains, hostname strategy, and TLS mode selection
- locales, currencies, timezones, and region policy
- storage policy defaults by path or folder
- feature flags and rollout configuration
- installed module settings and integration endpoints
- SEO defaults, redirects, and brand metadata

This is app-owned policy, not core-owned behavior.

## Content Models

The content model defines the structured shapes that the customer app edits and renders. Depending on the installed modules, that can include:

- pages and navigation structures
- localized fields and slugs
- branded landing-page blocks
- event or membership content types
- customer-specific resource fields layered onto official module data

The important rule is that the content model describes shape and editorial structure. It should not become a hidden workflow engine.

## When Configuration Is Enough

Configuration and schemas are the right home when the variation is about:

- allowed fields
- page structure
- locale-specific content
- brand-level policy
- routing or metadata defaults

If the variation changes business logic, data ownership, transaction flow, or integration behavior, it probably belongs in a module or extension instead.

## Versioning and Migration

Customer-app configuration and content-model changes should be versioned and migratable. This matters for long-lived apps because content structures, localized fields, and auth-linked resource types will evolve over time. The platform should treat these definitions as real application inputs, not as manually edited production state with no upgrade path.

## Interaction With Auth and Storage

Content models often introduce resources that need capability bindings, SEO output, caching rules, and storage defaults. Those cross-cutting concerns still resolve through core services. The customer app may define the schema and policy, but the execution remains within the platform's auth, cache, and storage model.
