# Public Docs Expansion Backlog

This backlog translates the current documentation feedback into a concrete implementation plan.

The goal of this pass is not to replace the current docs. It is to keep all current material and add the missing practical guidance that makes Coil usable by a real developer without forcing them to reverse-engineer the repository.

## Principles

- Keep all existing documentation content unless the replacement is clearly equivalent or better.
- Every page outside the marketing homepage must teach both `why` and `how`.
- Every abstract page must include at least one concrete example.
- Every reference page must include descriptions, examples, defaults, constraints, and practical guidance.
- Every major subsystem must link to a canonical Shoppr or Gitly implementation.
- If Shoppr or Gitly do not yet demonstrate a feature, add it to the demo first and then document it.

## Page Standard

Each non-homepage page should answer these questions in order:

1. What is this?
2. Why does it exist?
3. When should I use it?
4. How do I use it in Coil?
5. Which exact files, keys, APIs, templates, commands, or crates are involved?
6. What does a working example look like?
7. What are the constraints and common mistakes?
8. What should I read next?

Reference pages should also include:

- required vs optional
- default values
- allowed values
- field descriptions
- interactions with other settings
- copyable examples
- repo examples

## Current Structure

Published docs live under:

- `website/docs/getting-started`
- `website/docs/core-concepts`
- `website/docs/use-cases`
- `website/docs/operations`
- `website/docs/reference`
- `website/docs/contributing`

The current navigation is defined in:

- `website/sidebars.ts`

## Delivery Order

This is the execution order for the next pass.

1. Operations
2. Auth
3. Theme, template, internationalisation, accessibility, and SEO
4. Official module reference split
5. Linked Rust and WASM extension documentation
6. Shoppr and Gitly use-case enrichment
7. Contributing parity and final polish

## Dependencies Before Writing

Some pages need stronger demo coverage before the docs can become concrete enough.

### Demo Additions Required

- Shoppr custom migration example
- Shoppr custom metrics, tracing, and audit examples
- Shoppr jobs and scheduled-work examples
- Shoppr webhook and integration examples where missing
- Shoppr translation file examples
- Shoppr explicit dark, light, and system theme examples if current implementation is not clear enough
- Gitly background-work examples
- Gitly API route examples
- Gitly extension examples if current coverage is insufficient
- A clearly documented WASM host API surface if not already strong enough in code

### Acceptance Criteria For Demo Additions

- Each missing concept has one checked-in canonical implementation.
- That implementation is used directly by at least one docs page.
- The implementation is covered by tests where practical.

## Getting Started

The current quickstart flow is acceptable for now. Only additive cross-linking is needed later.

### `website/docs/getting-started/quickstart.md`

Status:
- Keep for now

Later additions:
- Link to customer project layout
- Link to operations build and deploy
- Link to Shoppr and Gitly use cases

Acceptance criteria:
- Quickstart remains short
- Quickstart points clearly into deeper docs

### `website/docs/getting-started/customer-project-layout.md`

Gaps:
- Needs stronger links into deeper explanations

Add:
- "Read next" section pointing to workspace, linked Rust backend, app.toml, platform config, and Shoppr

Acceptance criteria:
- A reader can move from starter understanding into detailed docs without guessing

### `website/docs/getting-started/linked-rust-backends.md`

Gaps:
- Needs stronger links into reference and use-case pages

Add:
- Links to linked Rust hook APIs, customer-root workspace, Shoppr linked backend, jobs, audit, observability

Acceptance criteria:
- This page becomes a launch point, not a dead end

## Core Concepts

These pages are narratively acceptable, but too abstract.

### `website/docs/core-concepts/index.md`

Gaps:
- Does not make the chapter structure explicit enough

Add:
- Short explanation of how to use Core Concepts vs Reference vs Use Cases
- Direct links to practical examples in Shoppr and Gitly

Acceptance criteria:
- Readers understand where conceptual learning ends and practical guidance begins

### `website/docs/core-concepts/glossary-and-mental-model.md`

Gaps:
- Needs more cross-links to concrete pages

Add:
- "Where to see this in practice" links for `site`, `locale`, `market`, `customer backend`, `extension`, `auth package`, `theme`, `cutover`

Acceptance criteria:
- Glossary terms are connected to implementation pages

### `website/docs/core-concepts/customer-root-workspace.md`

Gaps:
- Explains why, but not concretely enough how the workspace is assembled

Add:
- Full annotated Shoppr workspace tree
- Explanation of each crate and folder
- Concrete bootstrap example
- File links into Shoppr

Dependencies:
- None

Acceptance criteria:
- A developer can sketch their own workspace from this page alone

### `website/docs/core-concepts/runtime-and-module-composition.md`

Gaps:
- Missing practical composition examples

Add:
- Minimal composition example
- Manual composition example without `coil`
- Failure example when app manifest enables a module that the binary did not link

Dependencies:
- Module reference pages

Acceptance criteria:
- A developer knows how to choose between `coil` and selective composition

### `website/docs/core-concepts/request-and-render-lifecycle.md`

Gaps:
- Too conceptual

Add:
- Request trace for Shoppr home page
- Request trace for Shoppr product page
- Request trace for a state-changing route such as cart update
- Where auth, site resolution, locale, templates, SEO, linked Rust, and WASM participate

Dependencies:
- Template models page
- Theme structure page

Acceptance criteria:
- A developer can explain how a request becomes a response in Coil

### `website/docs/core-concepts/sites-locales-and-markets.md`

Gaps:
- Missing concrete multi-site examples

Add:
- Worked three-site Shoppr example
- Locale-only vs new-site decision matrix
- Inheritance and override rules with examples

Dependencies:
- Expanded `app.toml` reference

Acceptance criteria:
- A developer can decide when to add a locale and when to add a site

### `website/docs/core-concepts/customer-apps-vs-official-modules.md`

Gaps:
- Needs stronger ties to actual module enablement and extension points

Add:
- Shoppr example
- Gitly example
- Links to per-module reference pages
- Links to linked Rust and WASM reference pages

Acceptance criteria:
- A developer understands what belongs in core, module, customer app, linked backend, and extension

### `website/docs/core-concepts/themes-rendering-and-assets.md`

Gaps:
- Too abstract about layouts, fragments, pages, and asset delivery

Add:
- Annotated theme tree
- Explanation of layouts, fragments, pages, and assets in practice
- Explanation of why some templates carry full HTML structure
- Explanation of hashed assets and publication
- Section on JSON-LD and head metadata injection

Dependencies:
- Template models reference
- Theme structure reference

Acceptance criteria:
- A developer knows what files a theme contains and why

### `website/docs/core-concepts/internationalization-localization-and-content.md`

Gaps:
- Not practical enough

Add:
- Translation file locations
- Key naming patterns
- Template translation examples
- Fallback examples
- English, French, and Polish Shoppr examples

Dependencies:
- Internationalisation reference expansion

Acceptance criteria:
- A developer can add a new locale from this page

### `website/docs/core-concepts/accessibility-as-a-platform-contract.md`

Gaps:
- Too principle-oriented without enough Coil-specific guidance

Add:
- Practical markup examples
- What Coil validates
- What remains the app’s responsibility
- Shoppr or Gitly examples for forms, dialogs, tables, and navigation

Dependencies:
- Accessibility reference expansion

Acceptance criteria:
- A developer knows how to produce accessible Coil templates rather than just why accessibility matters

### `website/docs/core-concepts/seo-and-discoverability.md`

Gaps:
- Too abstract

Add:
- Worked example of canonical URL, alternate locales, robots, and JSON-LD
- Explanation of automatic vs custom SEO behaviour
- Links into Shoppr implementation and SEO reference

Dependencies:
- SEO reference expansion

Acceptance criteria:
- A developer understands which SEO behaviours are automatic and how to extend them

## Operations

This is the weakest section and needs the biggest pass.

### `website/docs/operations/project-organization.md`

Gaps:
- Too descriptive, not enough executable guidance

Add:
- Recommended project layouts
- When to use `coil`
- When to use selective dependencies
- How to add a new crate
- How to add a backend crate
- How to add an extension folder

Dependencies:
- Composition reference expansion

Acceptance criteria:
- A developer can organise a Coil repo correctly without guessing

### `website/docs/operations/build-and-deploy.md`

Gaps:
- Explains the lifecycle but not enough concrete execution detail

Add:
- Exact build commands
- Exact asset publication commands
- Exact migration commands
- Exact runtime start commands
- Production deployment example using Shoppr
- Production deployment example using Gitly
- Same-domain vs CDN asset serving guidance
- Custom schema migration example
- Guidance on customer-specific tables and versioning

Dependencies:
- Demo migration example
- Expanded platform config reference

Acceptance criteria:
- A developer can build and deploy a real Coil app from this page

### `website/docs/operations/configuration-and-secrets.md`

Gaps:
- Too advisory, not enough specific

Add:
- Full `platform.toml` and `platform.dev.toml` examples
- Explanation of each important block and field
- Secrets handling examples
- Environment variable examples
- Local vs production examples
- Links to exact reference sections

Dependencies:
- Platform config reference expansion

Acceptance criteria:
- A developer knows what to put in config, where, and why

### `website/docs/operations/observability.md`

Gaps:
- Lists concerns without showing the exact Coil surfaces

Add:
- Built-in logs, metrics, traces, health, readiness, and audit surfaces
- Definitions of each built-in metric and signal
- How to add custom metrics
- How to add custom traces
- How to add custom audit evidence
- Where to fetch audit evidence from
- Shoppr and Gitly examples
- Suggested dashboards grounded in actual Coil signals

Dependencies:
- Demo observability examples

Acceptance criteria:
- A developer can instrument and operate a Coil app without leaving the docs

### `website/docs/operations/jobs-and-schedulers.md`

Gaps:
- Too abstract about jobs

Add:
- How to define a job
- How to define a retryable job
- How to define a scheduled job
- How to define a domain-event-driven job
- Queue inspection and recovery commands
- Dead-letter handling commands
- Linked Rust examples from Shoppr or Gitly

Dependencies:
- Demo job examples

Acceptance criteria:
- A developer can add and operate background work in Coil

### `website/docs/operations/cache-tls-cutover-and-rollback.md`

Gaps:
- Too much theory in one page, not enough practical execution

Add:
- Exact cache topology guidance for `l1` and `l2`
- When one or both are needed
- Example cache configs
- TLS mode examples
- Exact cutover command flow
- Explanation of readiness checks
- Exact rollback flow
- Production checklists

Dependencies:
- Expanded platform config reference
- Possibly split into multiple pages if the page becomes too large

Acceptance criteria:
- A developer can prepare and run a production cutover from the docs

### `website/docs/operations/troubleshooting.md`

Gaps:
- Needs more symptom-driven guidance

Add:
- Sessions troubleshooting
- Assets troubleshooting
- Locale and site troubleshooting
- Extension troubleshooting
- Job troubleshooting
- Migration troubleshooting
- Cutover troubleshooting
- Webhook troubleshooting

Acceptance criteria:
- A developer can identify the right Coil subsystem to inspect for a concrete failure

### New Operations Pages

Add:
- `website/docs/operations/database-migrations.md`
- `website/docs/operations/webhooks-and-integrations.md`
- `website/docs/operations/health-readiness-and-maintenance-mode.md`
- `website/docs/operations/production-topologies.md`
- `website/docs/operations/asset-publication-and-cdn-delivery.md`

Acceptance criteria:
- These pages cover currently overloaded or missing operational detail

## Reference

This section is structurally good, but too many pages still assume the reader can infer semantics.

### `website/docs/reference/overview.md`

Add:
- Explain how to use reference pages
- Distinguish concepts, use cases, and reference

Acceptance criteria:
- A reader understands this section is the exactness layer

### `website/docs/reference/app-toml.md`

Gaps:
- Needs more field meaning and examples

Add:
- Descriptions for top-level blocks
- Minimal example
- Multi-site example
- Extension-enabled example
- Interactions between `sites`, `i18n`, `theme`, `auth`, `modules`, and `extensions`

Acceptance criteria:
- A developer can write a correct `app.toml` from this page

### `website/docs/reference/platform-config.md`

Gaps:
- Field coverage exists but guidance is too weak

Add:
- Meaning of each major field
- Required vs conditional vs optional
- Dev vs production examples
- Guidance on `cdn_base_url`
- Guidance on cache, session store, storage, TLS, and deployment mode

Acceptance criteria:
- A developer can write both `platform.dev.toml` and `platform.toml`

### Auth Subsection

#### `website/docs/reference/auth-overview.md`

Add:
- Real introduction to Coil auth
- RBAC comparison
- Zanzibar explanation in plain language

Acceptance criteria:
- A new reader is prepared for the rest of the auth docs

#### `website/docs/reference/auth-zanzibar.md`

Add:
- Tutorial treatment of Zanzibar
- Tuples, relations, usersets, inheritance, and computed relationships
- Real-world examples familiar to most developers

Acceptance criteria:
- A developer unfamiliar with Zanzibar can follow the rest of the auth docs

#### `website/docs/reference/auth-schema.md`

Add:
- Narrative explanation of primitives
- Where the vocabulary is used
- Whether and how custom vocabulary is added
- Worked examples combining relations and permissions

Acceptance criteria:
- A developer knows how to use the schema primitives

#### `website/docs/reference/auth-packages.md`

Add:
- Concrete package layout
- Exact `extend` and `replace` examples
- Capability binding explanations
- Validation and failure modes

Acceptance criteria:
- A developer can assemble a valid auth package

#### `website/docs/reference/custom-auth-schema.md`

Add:
- From-scratch walkthrough
- Store example
- Complete package example

Acceptance criteria:
- A developer can author a custom schema for their own domain

### `website/docs/reference/template-language.md`

Gaps:
- Major gap; still too assumption-heavy

Add:
- What the template language is
- Why it exists
- Exact syntax for directives and attributes
- Loops, conditionals, interpolation, fragments, slots, translation keys, escaping
- Valid and invalid examples
- Several complete Shoppr and Gitly examples

Acceptance criteria:
- A developer can write Coil templates from scratch

### `website/docs/reference/theme-structure.md`

Gaps:
- Too structural, not enough runtime meaning

Add:
- Explanation of layouts, fragments, pages, and models
- Built-in models and their fields
- Whether customer-defined models exist and how
- JSON-LD, OG, and head metadata guidance
- Explanation of full HTML templates

Dependencies:
- Template models page

Acceptance criteria:
- A developer understands what the theme tree means at runtime

### `website/docs/reference/internationalization.md`

Add:
- Translation file locations
- Translation syntax
- Locale fallbacks
- Site and locale interactions
- English, French, and Polish examples

Acceptance criteria:
- A developer can add translations without trial and error

### `website/docs/reference/accessibility.md`

Add:
- Practical markup examples
- Platform validations
- App responsibilities
- Common patterns and anti-patterns

Acceptance criteria:
- A developer can implement accessible Coil UI with confidence

### `website/docs/reference/seo.md`

Add:
- Exact metadata controls
- JSON-LD extension points
- Custom SEO markup example for a non-commerce domain
- Canonical, alternate, robots, sitemap, and OG examples

Acceptance criteria:
- A developer can customize SEO behaviour intentionally

### `website/docs/reference/modules.md`

This should become a hub page.

Add:
- Overview and links to dedicated module pages

New pages:
- `website/docs/reference/modules/cms.md`
- `website/docs/reference/modules/media.md`
- `website/docs/reference/modules/commerce.md`
- `website/docs/reference/modules/commerce-payments-stripe.md`
- `website/docs/reference/modules/memberships.md`
- `website/docs/reference/modules/events.md`
- `website/docs/reference/modules/admin.md`
- `website/docs/reference/modules/ops.md`

Each module page must cover:
- why it exists
- what it provides
- how to enable it
- how to disable it
- what config it expects
- what routes or surfaces it adds
- what auth capabilities it expects
- how customer apps extend it
- where Shoppr or Gitly use it

Acceptance criteria:
- A developer can understand and use each official module without reading code first

### `website/docs/reference/composition.md`

Add:
- Concrete Cargo examples
- `coil` vs selective dependency guidance
- Common composition patterns

Acceptance criteria:
- A developer can choose a composition strategy intentionally

### `website/docs/reference/customer-vs-wasm.md`

Current status:
- weakest reference page

Replace with additive expansion covering:
- linked Rust plugin model
- WASM extension model
- installation flow
- loading and unloading expectations
- instance model
- runtime lifecycle
- host integration
- what can and cannot be accessed
- packaging and distribution

Dependencies:
- WASM host API page
- linked Rust hook API page

Acceptance criteria:
- A customer developer can add linked Rust code
- A third-party developer can build and ship a WASM extension

### New Reference Pages

Add:
- `website/docs/reference/template-models.md`
- `website/docs/reference/theme-asset-delivery.md`
- `website/docs/reference/extension-package-format.md`
- `website/docs/reference/wasm-host-apis.md`
- `website/docs/reference/linked-rust-hook-apis.md`
- `website/docs/reference/cli-commands.md`
- `website/docs/reference/environment-variables.md`
- `website/docs/reference/migrations.md`

Acceptance criteria:
- All currently implied-but-underdocumented technical contracts are explicitly documented

## Use Cases

These pages should become the practical layer that turns abstract concepts into real Coil work.

### Shoppr Pages

Expand:
- `website/docs/use-cases/shoppr/overview.md`
- `website/docs/use-cases/shoppr/storefront-structure.md`
- `website/docs/use-cases/shoppr/catalog-and-merchandising.md`
- `website/docs/use-cases/shoppr/custom-pages-and-cms.md`
- `website/docs/use-cases/shoppr/sites-locales-and-theme-variants.md`
- `website/docs/use-cases/shoppr/linked-rust-backend.md`
- `website/docs/use-cases/shoppr/wasm-extensions.md`
- `website/docs/use-cases/shoppr/checkout-and-operations.md`

For every page, add:
- exact file locations
- annotated snippets
- expected runtime behaviour
- cross-links to reference pages
- direct “adapt this for your store” guidance

New Shoppr pages:
- `website/docs/use-cases/shoppr/observability-and-audit.md`
- `website/docs/use-cases/shoppr/jobs-webhooks-and-background-work.md`

Acceptance criteria:
- Shoppr becomes the canonical practical walkthrough for Coil commerce

### Gitly Pages

Expand:
- `website/docs/use-cases/gitly/overview.md`
- `website/docs/use-cases/gitly/product-structure.md`
- `website/docs/use-cases/gitly/theming-localization-and-accessibility.md`
- `website/docs/use-cases/gitly/api-and-background-work.md`

New Gitly pages:
- `website/docs/use-cases/gitly/extensions-and-host-apis.md`
- `website/docs/use-cases/gitly/build-and-deploy.md`

Acceptance criteria:
- Gitly proves the platform is not commerce-only and remains equally practical

## Contributing

### `website/docs/contributing/index.md`

Gaps:
- Too weak compared with repository-level docs

Add:
- Material equivalent to `CONTRIBUTING.md`
- Contribution flow
- issue guidance
- PR guidance
- standards
- where architecture docs fit
- links to code of conduct and security policy

Acceptance criteria:
- The docs site version stands on its own

## Cross-Linking Work

The docs currently do not connect pages strongly enough.

Add to every relevant page:
- `Reference:` links
- `Use case:` links
- `Canonical example:` file references
- `Read next:` section

Acceptance criteria:
- No major page ends without showing the reader where to go next

## Navigation Changes Needed

Once content exists, update:

- `website/sidebars.ts`

Changes expected:
- add new operations pages
- split official modules into per-module pages
- add new reference pages
- add new Shoppr and Gitly use-case pages

Acceptance criteria:
- Navigation matches the new practical depth
- Important pages are discoverable without search

## Definition Of Done

This docs pass is complete only when all of the following are true:

- A developer can configure, build, deploy, observe, extend, localize, theme, authorize, and operate a Coil app from the docs alone.
- Every abstract page contains at least one concrete example.
- Every reference page contains descriptions, defaults, examples, and practical guidance.
- Every major subsystem links to canonical Shoppr or Gitly implementations.
- Every “why” section is matched by a “how”.
- No current useful content has been removed unless replaced by equivalent or better material.

