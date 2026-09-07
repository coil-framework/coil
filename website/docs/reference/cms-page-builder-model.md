---
title: CMS Page Builder Model
---

This page explains the public page-builder boundary in Coil.

Use it when you need to answer questions like:

- what is the difference between a page type and a page record?
- what is a block definition versus a block instance?
- where does `page.blocks` come from?
- what part is editorial content versus request-time runtime shaping?

## The Model In One Sentence

Coil separates:

- content schema
- stored content instances
- request-time render-model shaping

Those are three different layers.

If you need the conceptual version first, read
[Content schema vs content instances](../core-concepts/content-schema-vs-content-instances/).

## Layer 1: Content Schema

Schema defines what editors are allowed to create.

Typical schema concepts:

- page types
- block types
- field definitions
- allowed block lists
- validation rules

Schema is not a page instance.

## Layer 2: Content Instances

Instances are the records that actually exist.

Typical instance concepts:

- a specific page record
- the page’s slug and publication state
- the ordered list of blocks on that page
- the field values stored on each block

Instances are where real editorial choices live.

## Layer 3: Request-Time Render Model

At request time, Coil still has to shape a template-facing model.

That model may include:

- page metadata
- block instances
- live data resolved from block config
- customer or module-owned request context

This is where templates get their final values.

## Page Type Vs Page Instance

### Page type

A page type is a schema concept.

Examples:

- `home`
- `landing_page`
- `article`

It defines:

- which fields exist
- which blocks are allowed
- which fields are required

### Page instance

A page instance is a content record.

Examples:

- `home-uk`
- `spring-sale`
- `about-us`

It carries real values such as:

- title
- slug
- status
- site or locale targeting
- ordered block instances
- page settings

## Structured Page Settings

Page settings are part of the content instance, not an afterthought hidden in the template.

Examples of page settings:

- page type
- template or layout hint
- SEO title
- SEO description
- navigation visibility
- indexing options

These settings are still editorial data. They are not infrastructure config, and they are not the
same thing as request-time render-model shaping.

## Block Definition Vs Block Instance

### Block definition

A block definition is schema.

Examples:

- `hero`
- `promo_grid`
- `faq`
- `featured_collection`

It defines the fields and validation rules for that kind of block.

### Block instance

A block instance is content.

Examples:

- the homepage hero for the UK site
- a `featured_collection` block for `spring-sale`
- an FAQ block containing five questions

It carries the real field values chosen by editors.

In a structured page-builder model, block instances should usually be treated as ordered structured
records, not one large free-form HTML blob.

## What `page.blocks` Means

`page.blocks` is a request-time render-model contract.

That is important because it is not guaranteed by schema alone.

For `page.blocks` to exist in a rendered page, Coil needs:

1. a route that resolves to a page
2. stored page and block instances
3. runtime shaping that exposes those blocks into the render model

If any one of those is missing, `page.blocks` does not appear automatically.

## Shared Blocks And Reusable Sections

If your content system supports reusable sections, treat them as content instances or content-backed
references, not as schema.

Examples:

- a reusable newsletter signup panel
- a reusable promo strip
- a reusable trust-and-delivery footer section

The important rule is the same:

- the reusable block definition is schema
- the reusable saved block is content
- the rendered output is request-time model data

In practice, that often means a page contains a mix of:

- page-local structured block instances
- shared block references

## Dynamic Blocks

Dynamic blocks add one more layer: live data resolution.

For example, an editor may configure:

- block type: `featured_collection`
- collection: `new-arrivals`
- limit: `8`

That stored block is still not the final UI.

The runtime must resolve the live product list and shape the request-time model.

For that pattern, read
[Dynamic blocks and live-data sections](../core-concepts/dynamic-blocks-and-live-data-sections/).

## What Coil Does Not Do Automatically

Coil does **not** automatically:

- create page instances because a page type exists
- create block instances because a block type exists
- populate `page.blocks` because a schema allows blocks
- resolve live data for dynamic blocks without runtime logic
- map every stored field into a template contract by default

You still need explicit request-time shaping.

For that pipeline, read
[Render pipeline and model composition](../core-concepts/render-pipeline-and-model-composition/).

## Safe Mental Model

Use this mental model:

- schema tells editors what they may create
- content instances store what they did create
- request-time shaping decides what templates can render

If you keep those layers separate, the CMS boundary stays understandable.

## See Also

- [app.toml](./app-toml/)
- [Template models](./template-models/)
- [Render model hooks](./render-model-hooks/)
- [Getting Started: Add a Real Content Model](../getting-started/add-a-real-content-model/)
- [Getting Started: Build Reusable Blocks](../getting-started/build-reusable-blocks/)
- [Getting Started: Add Dynamic Blocks](../getting-started/add-dynamic-blocks/)
