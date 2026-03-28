---
title: Implementation Gap Backlog
---

# Coil Implementation Gap Backlog

This backlog converts the current gap analysis into an execution plan.

It is intentionally split by:

- priority
- user-visible outcome
- design/doc alignment
- code/runtime work
- demo work

The goal is to close the biggest trust gaps first:

1. places where the docs/design describe capabilities that are not yet real
2. places where the public demos misrepresent the platform
3. places where core platform primitives exist but are not productised for customer developers

## Priority Definitions

- `P0`
  Platform credibility blockers. Public docs and demos should not continue to imply these are solved.
- `P1`
  Important product gaps that materially weaken the framework story but do not invalidate the whole platform.
- `P2`
  Useful improvements that make the platform more complete, teachable, or production-shaped.
- `P3`
  Refinements and polish after the higher-risk gaps are closed.

## Work Type Definitions

- `Design`
  ADR or design-doc updates needed because current docs overstate or under-specify the real target.
- `Code`
  Core/runtime/module/customer-SDK implementation work.
- `Docs`
  Public docs changes needed to match reality or document the finished implementation.
- `Demo`
  Shoppr/Gitly/example changes needed so the feature is visibly real.

## P0: I18n And Multi-Site Reality

### 1. First-class server-side translation catalogs

- Priority: `P0`
- Work types: `Design`, `Code`, `Docs`, `Demo`
- Problem:
  Core has locale context and `TranslationRuntime`, but there is no first-class customer-facing translation file contract and no end-to-end server-loaded translation story.
- Evidence:
  - [crates/coil-core/src/bootstrap/factories.rs](/Users/zcourts/projects/worka/coil/crates/coil-core/src/bootstrap/factories.rs)
  - [crates/coil-config/src/customer_app.rs](/Users/zcourts/projects/worka/coil/crates/coil-config/src/customer_app.rs)
  - [website/docs/reference/internationalization.md](/Users/zcourts/projects/worka/coil/website/docs/reference/internationalization.md)
- Desired outcome:
  A customer app can declare translation catalogs in a supported format, the runtime loads them, and server-rendered pages can resolve translated strings from those catalogs.
- Code work:
  - define customer translation file/directory format
  - extend customer app manifest/config to declare translations
  - load catalogs into `TranslationRuntime`
  - decide site-specific vs app-wide catalog layering
- Docs work:
  - replace “customer-owned convention” guidance with actual platform contract
  - document file format, fallback rules, lookup semantics, and examples
- Demo work:
  - Shoppr must use real server-side translation catalogs
  - Gitly can keep a smaller client-side demo, but it must not be the primary i18n story anymore
- Acceptance criteria:
  - translated server-rendered page copy exists in Shoppr
  - translation files are documented and validated
  - no public doc implies browser-only translation if the runtime supports more

### 2. Template translation primitive

- Priority: `P0`
- Work types: `Design`, `Code`, `Docs`
- Problem:
  There is no template-native translation helper such as `t()` or `coil:t`.
- Evidence:
  - [crates/coil-template/src/parser.rs](/Users/zcourts/projects/worka/coil/crates/coil-template/src/parser.rs)
  - [website/docs/reference/internationalization.md](/Users/zcourts/projects/worka/coil/website/docs/reference/internationalization.md)
- Desired outcome:
  Templates can render translated strings directly in a clear, constrained, documented way.
- Design decisions needed:
  - helper shape: expression helper `t("key")`, directive `coil:t`, or both
  - interpolation and pluralisation contract
  - error and fallback behaviour
- Code work:
  - parser support
  - runtime lookup support
  - escaping and fallback semantics
- Docs work:
  - full syntax page with examples and invalid cases
- Acceptance criteria:
  - translated template string rendering works server-side
  - helper is documented in template-language and i18n docs

### 3. Shoppr as a real multilingual multi-site demo

- Priority: `P0`
- Work types: `Code`, `Docs`, `Demo`
- Problem:
  Shoppr proves site-aware routing and assortment better than it proves translated content.
- Evidence:
  - [apps/shoppr/app.toml](/Users/zcourts/projects/worka/coil/apps/shoppr/app.toml)
  - [apps/shoppr/templates/pages/home.html](/Users/zcourts/projects/worka/coil/apps/shoppr/templates/pages/home.html)
  - [apps/shoppr/crates/shoppr-app/tests/sites.rs](/Users/zcourts/projects/worka/coil/apps/shoppr/crates/shoppr-app/tests/sites.rs)
- Desired outcome:
  Shoppr becomes the canonical proof that Coil can deliver:
  - multi-site
  - per-site locale policy
  - server-rendered translated copy
  - site-aware SEO
  - site-aware merchandising
- Demo work:
  - move shared shell copy out of hardcoded English
  - translate page copy for at least `en-GB`, `fr-FR`, `pl-PL`
  - keep site-specific assortment and branding visible
- Docs work:
  - update Shoppr walkthroughs to match the actual site set and locale set
- Acceptance criteria:
  - Shoppr home/category/product/cart pages render translated server copy by locale
  - tests assert translated output, not only host resolution

### 4. Remove `nip.io` as the default multi-site local-dev dependency

- Priority: `P0`
- Work types: `Design`, `Code`, `Docs`, `Demo`
- Problem:
  The current local multi-site story depends on external wildcard DNS and is therefore not self-contained.
- Evidence:
  - [apps/shoppr/platform.toml](/Users/zcourts/projects/worka/coil/apps/shoppr/platform.toml)
  - [apps/shoppr/README.md](/Users/zcourts/projects/worka/coil/apps/shoppr/README.md)
- Desired outcome:
  Local multi-site development works without relying on external public wildcard DNS.
- Candidate directions:
  - localhost + port/site mapping
  - explicit host-header dev tooling
  - generated local reverse-proxy config
  - first-class local site aliases managed by the dev server
- Acceptance criteria:
  - Shoppr multi-site demo works offline on a stock developer machine
  - docs do not require `nip.io`

## P0: Observability Reality

### 5. Real metrics implementation

- Priority: `P0`
- Work types: `Design`, `Code`, `Docs`, `Demo`
- Problem:
  Metrics are currently mostly catalog/config, not live telemetry.
- Evidence:
  - [crates/coil-runtime/src/server/observability.rs](/Users/zcourts/projects/worka/coil/crates/coil-runtime/src/server/observability.rs)
  - [crates/coil-observability/src/telemetry.rs](/Users/zcourts/projects/worka/coil/crates/coil-observability/src/telemetry.rs)
  - [website/docs/operations/observability.md](/Users/zcourts/projects/worka/coil/website/docs/operations/observability.md)
- Desired outcome:
  Live request/job/cache/storage/auth/extension metrics are emitted and scrapeable.
- Code work:
  - choose metrics backend/export contract
  - instrument runtime paths
  - expose real metric values
- Docs work:
  - document actual metrics, meanings, and usage
- Acceptance criteria:
  - `/metrics` or equivalent returns real values
  - docs describe implemented metrics only

### 6. Real tracing implementation

- Priority: `P0`
- Work types: `Design`, `Code`, `Docs`
- Problem:
  Trace flags exist, but live tracing/export does not match the docs/design.
- Desired outcome:
  Request/job/host-extension/storage/auth flows emit structured traces with documented propagation.
- Acceptance criteria:
  - live traces can be generated and exported
  - docs explain actual trace fields and setup

### 7. Honest and live readiness probes

- Priority: `P0`
- Work types: `Code`, `Docs`
- Problem:
  Readiness is bootstrap-shaped, not live dependency-shaped.
- Evidence:
  - [crates/coil-core/src/bootstrap/factories.rs](/Users/zcourts/projects/worka/coil/crates/coil-core/src/bootstrap/factories.rs)
  - [crates/coil-runtime/src/server/observability.rs](/Users/zcourts/projects/worka/coil/crates/coil-runtime/src/server/observability.rs)
- Desired outcome:
  `/ready` reflects real dependency reachability and essential runtime viability.
- Acceptance criteria:
  - DB/cache/object-store/queue failures affect readiness truthfully
  - docs describe the real probe contract

## P1: Demo And Public-Surface Integrity

### 8. Remove all remaining legacy Gitly names from Gitly

- Priority: `P1`
- Work types: `Code`, `Docs`, `Demo`
- Problem:
  Gitly still leaks old names in environment variables and code surfaces.
- Evidence:
  - [apps/gitly/crates/gitly-app/src/lib.rs](/Users/zcourts/projects/worka/coil/apps/gitly/crates/gitly-app/src/lib.rs)
  - [apps/gitly/docker-compose.yml](/Users/zcourts/projects/worka/coil/apps/gitly/docker-compose.yml)
  - [website/docs/reference/environment-variables.md](/Users/zcourts/projects/worka/coil/website/docs/reference/environment-variables.md)
- Desired outcome:
  Public surface is `Gitly` only.
- Acceptance criteria:
  - no public docs or default env names contain legacy Gitly branding
  - compatibility fallbacks, if retained, are documented as legacy-only

### 9. Fix Shoppr README/site walkthrough drift

- Priority: `P1`
- Work types: `Docs`, `Demo`
- Problem:
  Shoppr docs still mention site/locale combinations that do not exist.
- Evidence:
  - [apps/shoppr/README.md](/Users/zcourts/projects/worka/coil/apps/shoppr/README.md)
- Acceptance criteria:
  - README matches actual app manifest and tests exactly

### 10. Make Gitly’s multilingual story explicit and honest

- Priority: `P1`
- Work types: `Docs`, `Demo`
- Problem:
  Gitly is useful as a client-side dictionary demo, but it should not be mistaken for the platform boundary.
- Desired outcome:
  Gitly is documented as a secondary i18n pattern, not the primary platform story.

### 11. Replace browser-simulated jobs with a more real demo path

- Priority: `P1`
- Work types: `Code`, `Demo`, `Docs`
- Problem:
  Gitly’s visible job cadence is still mostly browser simulation.
- Desired outcome:
  At least one public demo should show a real runtime job/scheduler effect that is visible in the UI.

## P1: Platform Contract Gaps

### 12. General host-owned verified webhook pipeline

- Priority: `P1`
- Work types: `Design`, `Code`, `Docs`
- Problem:
  Verified webhook handling is narrower than the design/docs imply.
- Desired outcome:
  Webhook extension points are general host-owned ingress surfaces with verification, replay protection, retry, and dead-letter handling.

### 13. Broaden linked customer data facades

- Priority: `P1`
- Work types: `Design`, `Code`, `Docs`
- Problem:
  The customer-linked Rust facade surface is still partial relative to ADR 96 expectations.
- Desired outcome:
  Stable customer facades expose a broader supported data/workflow surface without forcing customer code into internals.

### 14. Unify durable audit story

- Priority: `P1`
- Work types: `Design`, `Code`, `Docs`
- Problem:
  Audit exists, but not as one clearly unified durable operator-history lane.
- Desired outcome:
  One documented audit contract for operator-relevant actions, with clear storage/durability semantics.

## P2: Reference And Docs Completeness

### 15. Split WASM docs into full public API reference pages

- Priority: `P2`
- Work types: `Docs`
- Problem:
  WASM docs are still too dependent on internal source references and under-specified for extension authors.
- Desired outcome:
  A third-party developer can build an extension from docs alone.
- Pages needed:
  - package format
  - writing/installing extensions
  - host service examples
  - grant vocabulary
  - lifecycle and loading

### 16. Split CLI docs into command-family reference pages

- Priority: `P2`
- Work types: `Docs`
- Problem:
  CLI overview exists, but command-family detail is still too compressed.
- Desired outcome:
  Each command family has exact usage, examples, and explanation of when to use platform CLI vs customer CLI.

### 17. Make every module extension section concrete

- Priority: `P2`
- Work types: `Docs`
- Problem:
  “How Customer Apps Extend It” sections are conceptually right but too vague.
- Desired outcome:
  Every official module page includes at least one concrete customer-owned extension example.

### 18. Migration docs must teach from examples, not source tours

- Priority: `P2`
- Work types: `Docs`
- Desired outcome:
  Migration docs show:
  - declaration
  - `migrate plan`
  - customer binary validate/apply behaviour
  - real outputs

## P2: Demo Fidelity

### 19. Shoppr should prove translated SEO and metadata

- Priority: `P2`
- Work types: `Code`, `Demo`, `Docs`
- Problem:
  Site-aware SEO exists, but translated content/metadata proof is weaker than needed.

### 20. Gitly should prove one real extension with non-empty host grants

- Priority: `P2`
- Work types: `Code`, `Demo`, `Docs`
- Problem:
  The current packages use empty grant sets, which keeps the examples simple but leaves host-API power under-demonstrated.

### 21. Shoppr asset/media realism

- Priority: `P2`
- Work types: `Demo`
- Problem:
  Some visible frontend pieces still use hardcoded remote image URLs, weakening the managed-asset story.

## P3: Later Refinements

### 22. Per-site locale restrictions in demos

- Priority: `P3`
- Work types: `Demo`, `Docs`
- Problem:
  Shoppr currently demonstrates site-aware routing better than site-specific locale policy.

### 23. Theme-state persistence beyond browser-local demo behaviour

- Priority: `P3`
- Work types: `Code`, `Demo`, `Docs`
- Problem:
  Gitly’s theme switching is useful, but purely frontend-local.

### 24. Synthetic/operator probe layer

- Priority: `P3`
- Work types: `Design`, `Code`, `Docs`
- Problem:
  The design mentions deeper synthetic checks, but they are not implemented.

## Recommended Execution Order

### Phase 1

- `P0-1` server-side translation catalogs
- `P0-2` template translation primitive
- `P0-3` Shoppr as real multilingual multi-site demo
- `P0-4` remove `nip.io` dependency for local multi-site

### Phase 2

- `P0-5` real metrics
- `P0-6` real tracing
- `P0-7` truthful readiness

### Phase 3

- `P1-12` general verified webhook pipeline
- `P1-13` broaden linked customer facades
- `P1-14` unify audit story

### Phase 4

- `P1-8` remove remaining legacy Gitly names
- `P1-9` fix Shoppr walkthrough drift
- `P1-10` clarify Gitly multilingual role
- `P1-11` real runtime job demo

### Phase 5

- `P2-15` WASM docs split
- `P2-16` CLI docs split
- `P2-17` module extension examples
- `P2-18` migration docs examples
- `P2-19` translated SEO demo
- `P2-20` non-empty grant demo
- `P2-21` managed asset realism

## Definition Of Done

This backlog is only complete when:

- the public docs describe only real capabilities
- Shoppr proves multi-site plus server-rendered multilingual content
- Gitly proves non-commerce platform versatility without becoming the accidental i18n reference model
- observability claims are backed by actual telemetry, not only config toggles or catalogs
- local multi-site dev works without external wildcard DNS dependence
- extension authors and customer developers can succeed from the docs without reading core implementation first
