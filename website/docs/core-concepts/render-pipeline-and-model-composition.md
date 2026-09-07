---
title: Render Pipeline And Model Composition
---

This page is the canonical explanation of how Coil gets from an incoming request to a template with
data.

If you are asking:

- where does the final render model come from?
- what part does the framework own?
- what part can official modules shape?
- where does customer code enter?

this is the page to read.

## The Five-Step Pipeline

The request-time render path is:

1. route resolution
2. framework base render model
3. official-module contributions
4. customer contributions
5. template rendering

That is the core pipeline.

## 1. Route Resolution

Coil first resolves the request into a runtime route target.

That gives the system enough context to know things like:

- route name
- template name
- locale
- site
- route params
- whether the request is a page or fragment

This step decides **what is being rendered**.

It does not yet decide the full model shape.

## 2. Framework Base Render Model

Once the route is known, Coil builds the common request model that most templates expect.

This usually includes shared request-time values such as:

- app and route metadata
- site and locale context
- path and route params
- navigation and links
- page shell metadata
- flash messages and request status data

This step gives templates a predictable outer shell.

The important boundary is:

> Coil provides the base request model. It does not guess your customer-specific page data.

## 3. Official-Module Contributions

Official modules then contribute route-specific model data for the routes they own.

Examples:

- commerce shaping a product detail page
- CMS shaping a page editor view
- admin shaping an order detail screen
- memberships shaping an account summary

This is where the framework-owned model becomes more specific.

Official modules are allowed to populate framework contracts such as:

- `page`
- `navigation`
- `product`
- `order`
- `membership_summary`

depending on the route being rendered.

## 4. Customer Contributions

After the base model and official-module data exist, customer code can contribute additional model
data.

Today the public supported customer lane is render-model hooks from linked Rust.

Those hooks can do two things:

- mount a new top-level namespace
- merge fields into an existing shared object

Examples:

- mount `crm_page`
- mount `marketing_banner`
- merge additional fields into `page`

This is the supported answer to "where does my custom request-time model logic live?"

For the exact API, read [Render model hooks](../reference/render-model-hooks/).

## 5. Template Rendering

Only after the previous steps are complete does Coil render the final template.

Templates do not go back and compute the missing model themselves.

Templates read the final request-time model that the previous steps produced.

That means a template is the last stage in the pipeline, not the source of truth for the data.

## What Coil Does Not Do Automatically

This is the most important clarification for public docs.

Coil does **not** automatically:

- convert `app.toml` content model declarations into live `page.blocks`
- infer customer page data just because a template references a field
- populate dynamic sections because a schema allows them
- merge arbitrary customer data into framework-owned objects without an explicit hook
- turn a block definition into a live fragment render on its own

You still need explicit request-time shaping from:

- framework base model code
- official modules
- customer render-model hooks

## Mount Vs Merge

Customer contributions are intentionally split into two modes.

### Mount

Mount creates a new top-level namespace.

Use it when:

- the data is customer-owned
- you want a clear boundary
- you do not want to collide with framework keys

Example outcome:

```text
crm_page.hero_title
```

### Merge

Merge contributes fields into an existing shared object.

Use it when:

- the field belongs in a shared contract such as `page`
- customer code is intentionally participating in the existing model
- conflicts should be checked explicitly

Example outcome:

```text
page.render_source
page.campaign_label
```

The important rule is:

> customer code should not treat framework-owned objects as an unbounded dumping ground.

Mount when the data is truly customer-owned. Merge only when the shared object is the correct public
contract.

## Framework Namespaces Vs Customer Namespaces

Coil already owns parts of the request model.

Examples of framework or module-shaped namespaces include:

- `page`
- `site`
- `links`
- `navigation`
- `product`
- `order`

Customer code should generally prefer a customer-owned namespace for new data:

- `crm_page`
- `campaigns`
- `recommendations`
- `customer_extension`

That keeps the boundary clear and reduces upgrade risk.

Merge into framework objects only when that field genuinely belongs to the public shared contract.

## Example: CMS Landing Page With Customer Personalisation

A request for `/spring-sale` might be shaped like this:

1. route resolution chooses `cms.page-detail`
2. base model adds `route_name`, `site`, `links`, and `page` shell data
3. CMS module contributes the page instance and editorial block data
4. customer hook mounts `crm_page` with audience-specific banner data
5. template renders both `page.blocks` and `crm_page.hero_variant`

That is a composed request-time model.

No single file creates the whole page by itself.

## Common Mistakes

### Treating the template as the data source

Templates render. They do not define the real request-time model contract by themselves.

### Treating `app.toml` as request-time render logic

`app.toml` expresses app composition, not the full request-time page model.

### Treating customer hooks as a replacement for route ownership

Customer hooks shape model data. They do not replace route resolution, official module ownership, or
the template contract.

## Related Concepts

- [Content schema vs content instances](./content-schema-vs-content-instances/)
- [Dynamic blocks and live-data sections](./dynamic-blocks-and-live-data-sections/)
- [Request and render lifecycle](./request-and-render-lifecycle/)

## Reference Pages

- [Template models](../reference/template-models/)
- [Render model hooks](../reference/render-model-hooks/)
- [Getting Started: Add Dynamic Blocks](../getting-started/add-dynamic-blocks/)
