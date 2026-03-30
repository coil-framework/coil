---
title: Dynamic Blocks And Live-Data Sections
---

Dynamic blocks are page sections whose final output depends on both editorial configuration and
runtime data.

This is where people often expect Coil to do more automatically than it actually does.

## The Short Version

A dynamic block usually has three parts:

1. editorial configuration stored as content
2. runtime logic that resolves live data
3. a template or fragment that renders the final output

If any one of those parts is missing, the block is not actually dynamic.

## What Makes A Block Dynamic

A static block can often render directly from stored content:

- heading
- body text
- image
- CTA

A dynamic block needs additional runtime work.

Examples:

- featured products based on current catalog state
- upcoming events based on live schedules
- membership summary based on account state
- campaign banners driven by site, locale, or audience
- live admin or dashboard cards

The editor may choose that a block belongs on the page, but the runtime still has to resolve the
actual data shown inside it.

## The Three-Layer Model

### 1. Editorial configuration

This is the CMS-owned part.

Examples:

- block type is `featured_collection`
- collection handle is `spring-sale`
- title override is `Fresh arrivals`
- max items is `6`

This tells Coil what the editor wants the section to do.

### 2. Runtime logic

This is the request-time part.

Examples:

- load six visible products from the current site catalog
- filter events by locale and publish state
- load the current customer membership status
- decide whether the current site should show the block at all

This logic usually lives in:

- official-module model shaping
- customer render-model hooks
- fragment handlers or extension handlers

### 3. Template or fragment rendering

This is the presentation layer.

Examples:

- render a product grid
- render an event list
- render a membership badge strip

The template only works once the runtime has shaped the final request-time values.

## What Coil Does Not Do Automatically

Coil does **not** automatically:

- turn a block schema into a live data query
- populate a dynamic section just because an editor chose that block type
- infer which repository or API should back a block
- decide how block config maps to runtime queries
- dispatch fragment rendering for a block unless the runtime or handler path is wired explicitly

That work must be designed.

## Example: Featured Products Block

Imagine a page builder lets editors place this block:

```text
type = featured_collection
collection = spring-sale
limit = 6
title = Fresh arrivals
```

That config is not yet a rendered block.

The runtime still needs to:

1. read the block instance
2. resolve the current site and locale
3. fetch the matching visible products
4. shape a request-time model
5. render the block template

Without that runtime step, the page only knows that the editor requested a featured collection. It
does not know which six products to show.

## Dynamic Blocks Vs Live-Data Sections

These terms are closely related but not identical.

- a **dynamic block** is a block-shaped editorial unit whose output depends on runtime logic
- a **live-data section** is the broader concept of any request-time section backed by live state

In practice, a live-data section often appears inside a dynamic block system, but it can also appear
as a route-owned section outside a page builder.

## Where The Data Should Live

Use this rule of thumb:

- content editors own layout choices and editorial configuration
- runtime code owns live data loading and state interpretation
- templates own final presentation

Do not push runtime data loading into content schema. Do not push editorial block configuration into
low-level runtime code unless the page is entirely route-owned and not editor-driven.

## Common Mistakes

### Treating dynamic blocks as just another static CMS block

Static blocks and dynamic blocks are not the same operationally. Dynamic blocks require request-time
logic.

### Expecting `page.blocks` to include fully resolved live data automatically

Whether a block exposes only editorial config or a fully shaped live payload is a runtime contract,
not an automatic platform guarantee.

### Hiding all live behavior inside templates

Templates should render already-shaped data. They should not become the place where runtime data
loading or dispatch logic is secretly implemented.

## Where To Look Next

- [Content schema vs content instances](./content-schema-vs-content-instances.md)
- [Render pipeline and model composition](./render-pipeline-and-model-composition.md)
- [CMS page builder model](../reference/cms-page-builder-model.md)
- [Render model hooks](../reference/render-model-hooks.md)
- [Getting Started: Build Reusable Blocks](../getting-started/build-reusable-blocks.md)
- [Getting Started: Add Dynamic Blocks](../getting-started/add-dynamic-blocks.md)
