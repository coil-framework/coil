---
title: Content Schema Vs Content Instances
---

Coil separates **content schema** from **content instances**.

That distinction matters because a schema file is not the same thing as the actual page, block, or
entry data that a request can render.

## The Short Version

- content schema defines what kinds of content can exist
- content instances are the actual records and values that do exist
- templates render request-time instances, not schema declarations

If you keep only one rule in mind, keep this one:

> A schema declares allowed structure. It does not populate page content by itself.

## What Counts As Schema

Schema is the contract layer.

Examples:

- page types such as `home`, `landing_page`, or `article`
- block types such as `hero`, `promo_grid`, or `faq`
- field definitions such as `title`, `summary`, `cta_label`, or `background_image`
- editorial rules such as which blocks can appear inside which page types

Schema answers questions like:

- what fields are valid here?
- what blocks are allowed here?
- which fields are required?
- how should editors structure content?

Schema is about **possibility**, not **actual content**.

## What Counts As A Content Instance

Content instances are the real values that exist in the app.

Examples:

- a specific homepage record for the UK site
- a specific `hero` block on that homepage
- a specific `promo_grid` block containing three campaign cards
- a specific article with its current title, body, and publish state

Instances answer questions like:

- what is the homepage title right now?
- which blocks are on this page today?
- which call-to-action text is live for the French site?

Instances are about **state and values**, not **allowed shape**.

## Why The Separation Exists

Coil keeps these layers separate so that:

- editors can change content without changing app composition
- developers can change structure without pretending structure is data
- render logic can stay explicit about what is available at request time

This is also why `app.toml` is not a page builder.

`app.toml` can declare content models and migration ownership, but it does not itself create page
instances, populate CMS records, or inject live `page.blocks` into the render model.

For the manifest contract, read [app.toml](../reference/app-toml/).

## What Coil Does Not Do Automatically

These are the common wrong assumptions:

- defining a page type does not create a page instance
- defining a block type does not make that block appear on a page
- listing a content model in `app.toml` does not populate request-time `page.blocks`
- adding a field to a schema does not automatically map it into a template variable

You still need:

- stored content instances
- runtime logic that loads or shapes those instances into the render model
- templates that read the resulting model keys

If any one of those three layers is missing, the page will not fill itself in.

## Example: Product Landing Page

Imagine you define this editorial concept:

- page type: `landing_page`
- allowed blocks: `hero`, `promo_grid`, `faq`
- required fields: `title`, `slug`

That schema tells Coil what a valid landing page looks like.

It does **not** mean the request to `/spring-sale` will automatically render:

- a `hero` block
- three promo cards
- an FAQ section

For that to happen, you still need actual stored content like:

- page instance: `spring-sale`
- block instance 1: `hero`
- block instance 2: `promo_grid`
- block instance 3: `faq`

And then runtime code has to shape those instances into the request-time model the template reads.

## Where Content Instances Enter The Render Path

At render time, Coil works with a request model, not a schema file.

The flow is:

1. a route resolves
2. Coil builds the base request model
3. official modules and customer code contribute request-time data
4. templates render the final combined model

That means content instances only matter once they have been loaded or composed into the render
model.

For the canonical request-time flow, read
[Render pipeline and model composition](./render-pipeline-and-model-composition/).

## Dynamic Blocks Make This Even More Important

Dynamic blocks often make the schema/instance boundary harder to see.

A dynamic block usually includes:

- editorial configuration stored as content
- runtime logic that resolves live data
- a template or fragment that renders the result

So even when the editor has chosen a block, the live block output still depends on request-time
runtime work.

For that model, read
[Dynamic blocks and live-data sections](./dynamic-blocks-and-live-data-sections/).

## Common Mistakes

### Treating `app.toml` as a CMS database

`app.toml` can describe app composition and content ownership boundaries. It is not where live page
instances are stored.

### Expecting schema changes to update templates automatically

Templates only know about the render model they receive. A new schema field is not usable until the
request-time model exposes it.

### Expecting `page.blocks` to appear because blocks exist conceptually

`page.blocks` is a request-time model decision. It exists only if runtime code or hooks populate it.

## Read Next

- [Render pipeline and model composition](./render-pipeline-and-model-composition/)
- [Dynamic blocks and live-data sections](./dynamic-blocks-and-live-data-sections/)
- [app.toml](../reference/app-toml/)
- [CMS page builder model](../reference/cms-page-builder-model/)
- [Getting Started: Add a Real Content Model](../getting-started/add-a-real-content-model/)
- [Getting Started: Build Reusable Blocks](../getting-started/build-reusable-blocks/)
