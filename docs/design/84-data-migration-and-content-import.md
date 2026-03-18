# Data Migration and Content Import

**Part:** Migration and Evolution  
**Chapter:** 84

The platform treats import as a first-class operational workflow. Content migration is not a one-shot script that runs once and disappears. It is a repeatable pipeline used for initial loads, delta imports during parallel run, and corrective reruns when mappings change. That implies idempotency, auditability, and clear ownership of each imported resource.

## Import Principles

Every import process should satisfy four rules:

- source identifiers are retained on imported records so reruns can update rather than duplicate
- imports are chunked and resumable
- validation happens before publication
- import state is visible to operators and support staff

These rules matter equally for pages, users, products, subscriptions, events, bookings, and assets. The platform should not have a “special one-off” import path for each module family.

## Import Package Shape

An import run should be described by a manifest that names:

- source system and snapshot time
- target customer app and module set
- mapping rules
- storage policy defaults for assets
- locale and site context
- validation mode and publication mode

The manifest is the operator-facing contract. The import engine can then hand work to module-specific importers for CMS content, media, memberships, or events while preserving one audit trail.

## Content Transformation

Legacy content nearly always needs structural change. WordPress flexible-content pages, plugin metadata, or custom post types should be transformed into typed blocks, fields, and managed resources understood by the new platform. That transformation layer is responsible for:

- normalizing locale and slug handling
- mapping legacy HTML or block structures into supported template or page-composer structures
- splitting attachment metadata from the binary object itself
- translating implicit status flags into explicit publish states and capabilities

Import is therefore part data conversion and part model enforcement. If the source contains states the target model does not support, the importer should stop or stage those records for manual review instead of silently degrading them.

## Media and Asset Import

Media import must distinguish deployment artifacts from managed assets. Deployment artifacts, such as hashed CSS or JavaScript bundles, are published by the build pipeline and are not imported. Managed assets, such as uploaded files, product photography, event images, documents, or downloadable member resources, are imported into the asset system with:

- storage policy assignment
- publication state
- source checksum or etag where available
- source URL or source object identifier
- metadata for locale, copyright, alt text, and owner

If the imported asset is publishable, the importer must still respect auth and capability rules. Importing a file is not the same as publishing it publicly.

## Structured Domain Imports

Some domains require ordered migration:

- users and groups before memberships or subscriptions
- products before orders
- events and timeslots before reservations and bookings
- pages before navigation structures that link to them

Module importers should therefore declare dependencies. The import runner can then execute them in phases and block publication until the dependency graph is satisfied.

## Idempotency and Verification

Idempotency is not just “safe to rerun.” It also means the operator can answer what happened during the rerun. Each imported record should retain:

- the source system key
- the import batch identifier
- the last-seen checksum or fingerprint
- the target record identifier
- the result status

Verification should include record counts, required-field validation, route resolution checks, auth-binding checks where content has publication permissions, and storage validation for media objects.

## Example: Importing Events From WordPress

A WordPress events import typically reads event posts, post meta, taxonomies, uploaded media, and booking-related tables. In the new platform that becomes:

- event records in the events module
- time-slot or capacity records in the scheduling layer
- booking or reservation records only if the target system is ready to own them
- managed assets for hero images, attachments, and passes
- redirect rules for old event URLs

The importer should not attempt to preserve every internal plugin artifact. It should preserve customer-visible behavior and the data required for the new events-and-memberships app to operate.

## Production Readiness

Imported data is considered production-ready only when:

- module-level validation passes
- sample user journeys succeed against the imported data
- redirects, canonical URLs, and structured metadata render correctly
- media objects resolve under their intended storage and delivery policy
- the customer team has reviewed any staged exceptions

Until then, import is staging work, not a launch artifact.
