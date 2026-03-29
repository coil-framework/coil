# Composable Page Builder and Editorial Model

## Purpose

Coil needs a first-class editorial system for retail and membership-led sites whose marketing, brand, landing, event, and account-adjacent pages are assembled dynamically from reusable content blocks. The goal is not simple static page rendering. The goal is to preserve editorial flexibility while keeping the runtime and customer extension model explicit.

## Capability Summary

The platform must support:

- structured block type definitions
- structured page block instances
- per-page block ordering
- block enable and disable state
- reusable shared blocks
- nested and repeatable field groups
- page-level settings and targeting
- global settings and options pages
- live-data blocks that combine editorial configuration with runtime state
- a previewable render pipeline that makes the boundary between schema, content, and rendering explicit

## Model Layers

The editorial system must distinguish these layers clearly.

### 1. Block Type Schema

Block type schema defines:

- block identity
- field definitions
- field validation
- nested field structure
- defaults
- editor labels and help text
- allowed placement rules if needed

This is design-time schema. It describes what blocks can exist. It does not define which blocks appear on a specific page.

### 2. Page and Block Instances

Page instances define:

- which blocks are present on a page
- ordering
- enabled and disabled state
- per-instance field values
- visibility rules
- site, locale, audience, or campaign targeting overrides

This is authorable content. It is the missing layer between schema and templates in most CMS migrations.

### 3. Shared and Reusable Blocks

Editors need reusable content primitives for:

- recurring banners
- campaign modules
- membership calls to action
- legal or informational footers
- reusable branded sections

The platform should support:

- embedding a shared block by reference
- choosing whether a page instance snapshots or references the shared block
- previewing where a shared block is used
- safe publish semantics when a shared block changes

### 4. Render Model Shaping

The final template-visible model is runtime output, not raw CMS storage.

The runtime must:

- load the page instance
- resolve enabled blocks
- resolve reusable block references
- apply site and locale targeting
- combine editorial fields with live data where needed
- expose a stable render model to templates

This step is where customer render-model hooks participate.

## Page-Level Settings

The editorial system must support page-level settings beyond block lists.

Required examples:

- header and footer variants
- body or layout classes
- top and bottom spacing
- hide or show header/footer chrome
- page-level hero or branding overrides
- region override
- locale restrictions if needed
- audience gating
- membership gating
- redirect or upgrade behavior for unauthorized access

These settings are editorial product features, not just theme concerns.

## Global Settings and Options Surfaces

Coil needs a first-class options system for shared content and shared configuration used by templates and operational flows.

Required categories include:

- footer content
- social and contact details
- modal content
- legal copy
- brand-specific copy and assets
- scheduler switches for editorial-facing reminders
- payment and pass messaging
- white-label or partner-specific settings

The platform should not force these into ad hoc TOML blobs or code constants when they are actively edited by non-developers.

## Static Blocks vs Dynamic Blocks

The platform must explicitly support two classes of blocks.

### Static Blocks

Static blocks render entirely from stored editorial fields.

Examples:

- hero
- text area
- FAQ accordion
- icon columns
- two-column content

### Dynamic Blocks

Dynamic blocks combine stored editorial configuration with live application state or queries.

Examples:

- featured events
- memberships table
- account-state-aware promotional sections
- product or inventory carousels
- personalized recommendations

Dynamic blocks require:

- schema-owned configuration fields
- runtime-owned query and shaping logic
- a stable fragment or component contract

These should not be treated as a hack layered on top of static page rendering.

## Rendering Contract

The rendering pipeline must be documented and implemented as:

1. load page instance
2. resolve global options relevant to the request
3. resolve block references and visibility rules
4. construct base framework render model
5. allow customer render-model contributions
6. expose canonical page and customer-owned namespaces to templates
7. dispatch block fragments or loop over blocks explicitly

This means Coil must support both:

- a customer-owned namespace such as `customer.page.blocks`
- selective merges into framework-owned models such as `page.blocks`

Default conflict behavior when merging must remain fail-closed.

## Editorial Workflow

The CMS must support:

- draft
- preview
- scheduled publication
- live publication
- revisions
- rollback
- publish audit trail

For page-builder content specifically, previews must render the full page-instance shape, not just individual fields.

## Admin Authoring Requirements

Editors need an authoring experience that supports:

- add block
- remove block
- reorder block
- disable block without deleting it
- duplicate block
- insert shared block
- edit nested field groups
- preview page as composed
- understand which pages use a shared block

This is essential if Coil is meant to replace established editorial workflows without reducing flexibility.

## Required Official Module Boundaries

The recommended module split is:

- `coil-cms`
  owns schema definitions, page instances, options surfaces, publication, previews, and shared blocks
- `coil-admin`
  owns the editorial authoring interface and operational tables/forms
- `coil-runtime`
  owns render-model assembly and template rendering
- customer linked Rust
  owns customer-specific dynamic block data shaping and bespoke editorial logic

## Non-Goals

The platform should not:

- treat structured page composition as raw template editing
- collapse schema and content instances into the same file format
- rely on templates to perform domain logic that belongs in customer hooks or official modules
- hide live-data block behavior behind undocumented globals

## Immediate Implications for Coil

Coil’s current CMS shape is not enough if it stops at block schemas and template dispatch. To satisfy this requirement, Coil needs:

- structured page-instance storage
- reusable block storage and reference semantics
- page settings beyond route metadata
- options surfaces
- dynamic block contracts
- editor-facing composition UI
