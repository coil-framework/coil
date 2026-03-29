# Platform Boundaries and Public Documentation Gaps

## Purpose

The current public documentation explains many internal pieces correctly, but it does not yet tie them together clearly enough for customer developers building complex sites. The result is that a reasonable developer can understand the individual parts while still misunderstanding how they are meant to compose.

This document identifies the most important boundary clarifications and the documentation changes needed to make those boundaries clear.

## The Main Boundary Problem

The current gap is not lack of individual API reference. The gap is that the platform seams are still too implicit in the documentation.

Developers need a direct explanation of:

- what belongs in configuration
- what belongs in authored content
- what belongs in render-model shaping
- what the framework provides automatically
- what customer linked Rust must contribute explicitly

Without that, developers naturally assume the platform behaves more magically than it does.

## Boundary Clarifications the Public Docs Must Make Explicit

### 1. Schema Is Not Content

The docs must state plainly:

- block schema defines what can exist
- page or block instances define what actually exists on a request
- templates render the request-time model, not the schema

This distinction should be taught before showing template examples.

### 2. `app.toml` Does Not Populate the Render Model

The docs must explicitly say:

- `app.toml` defines application structure and schema
- it does not by itself populate `page.blocks` or customer-specific top-level template data
- request-time data must come from official modules or customer render-model hooks

This is one of the easiest places for developers to form the wrong mental model.

### 3. The Render Pipeline Needs One Canonical Explanation

The public docs need a dedicated page that walks through:

1. route resolution
2. framework base render model construction
3. official-module contributions
4. customer render-model contributions
5. template rendering

That page should show where mount and merge happen, and which side owns which model prefixes.

### 4. Customer Namespaces vs Framework Models

The docs must clearly explain when to:

- mount under a customer-owned namespace
- merge into framework-owned objects such as `page`

Recommended public guidance:

- mount for customer-specific or domain-specific data
- merge only when intentionally participating in a shared framework contract
- default to namespacing for clarity and safety

### 5. Dynamic Blocks Need an Explicit Contract

The docs should define dynamic blocks as a first-class concept:

- editorial configuration stored in the CMS
- runtime logic in customer or official-module code
- fragment dispatch or shaped render-model output at render time

Developers should not have to infer this from templates alone.

### 6. Official Modules vs Customer Code

The docs should include a concrete capability matrix that answers:

- which parts of CMS are official-module concerns
- which parts of booking are official-module concerns
- which parts of integrations belong in customer linked Rust
- which parts of admin are shared shell primitives

## Proposed Public Documentation Additions

The public site should gain the following documents or major expansions.

### Core Concepts

- `content-schema-vs-content-instances.md`
  Explains schema, instances, and publication.
- `render-pipeline-and-model-composition.md`
  Explains request-to-template composition with official and customer contributions.
- `dynamic-blocks-and-live-data-sections.md`
  Explains how editorial blocks combine with runtime data.

### Reference

- expand `app-toml.md`
  Add explicit statements about what it does not do.
- expand `render-model-hooks.md`
  Add end-to-end customer examples showing mount and merge.
- expand `template-models.md`
  Explain canonical framework models vs customer-owned prefixes.
- add `cms-page-builder-model.md`
  Define schema, block instances, shared blocks, and page settings.

### Use-Case Guides

- add a guide for building a dynamic marketing page with shared blocks and live event data
- add a guide for content plus booking integration
- add a guide for audience-gated pages and customer-specific rendering

## Documentation Style Changes Needed

The public docs should shift in three ways.

### 1. Lead With System Shape, Then API Detail

The docs currently tend to explain APIs and internals before the whole composition model is anchored. For customer developers, the order should be:

1. conceptual model
2. system boundaries
3. end-to-end example
4. API reference

### 2. Prefer End-to-End Examples Over Isolated Snippets

Examples should show the full chain:

- block schema
- content instance
- render-model hook
- mounted or merged model
- template usage

That is the level where the real joints become visible.

### 3. State Non-Goals Explicitly

Where a feature does not behave implicitly, the docs should say so directly.

Examples:

- `app.toml` does not populate request-time content instances
- template expressions do not replace business logic
- render-model hooks do not define schema

## Immediate Documentation Priority

Before more public examples are added, Coil should document these boundaries clearly enough that a customer developer can answer:

- where do my block definitions live
- where do my page instances live
- where does request-time data get shaped
- how do I expose that model to templates
- when should I mount under my own prefix vs merge into `page`

Until those questions are answered directly, the docs will continue to feel technically correct but compositionally incomplete.
