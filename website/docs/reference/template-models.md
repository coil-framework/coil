---
title: Template Models
---

Davenda templates render against a typed `RenderModel`, not an unstructured JSON blob.

## Start With A Model And Its Template

Imagine runtime code shaping this model:

```rust
let model = RenderModel::new()
    .with_value("locale", RenderValue::text("en-GB"))?
    .with_object(
        "site",
        RenderModel::new()
            .with_value("brandName", RenderValue::text("Shoppr"))?
            .with_value("displayName", RenderValue::text("Shoppr UK"))?,
    )?
    .with_bool("hasFlashMessages", true)?
    .with_list(
        "flashMessages",
        vec![
            RenderModel::new()
                .with_value("text", RenderValue::text("Order updated"))?
                .with_value("level", RenderValue::text("info"))?,
        ],
    )?
    .with_asset_path("theme/assets/site.css", "https://cdn.example.com/theme/assets/site.abc123.css")?;
```

And this template consuming it:

```html
<html xmlns:dv="https://davenda.dev" dv:attr="lang=${locale}">
  <head>
    <link rel="stylesheet" dv:href="asset('theme/assets/site.css')" />
  </head>
  <body>
    <h1 dv:text="${site.brandName}">Brand</h1>
    <section dv:if="${hasFlashMessages}">
      <article dv:each="message : ${flashMessages}">
        <p dv:text="${message.text}">Fallback</p>
      </article>
    </section>
  </body>
</html>
```

That is the core contract:

- runtime shapes typed values
- templates read those values declaratively

## What Types Exist?

The core types are:

- `RenderModel`
  - map of keys to values plus an asset-path map
- `RenderValue::Text`
- `RenderValue::TrustedHtml`
- `RenderValue::Bool`
- `RenderValue::List(Vec<RenderModel>)`
- `RenderValue::Object(RenderModel)`

This matters because templates are not dynamically evaluating arbitrary JSON. They are reading a
small, typed value model.

## How Templates Consume The Model

These are the important rules:

- `${page.title}`
  - reads nested object keys
- `dv:if="${hasFlashMessages}"`
  - expects a boolean
- `dv:each="entry : ${auditEntries}"`
  - expects a list of child models
- `asset('theme/assets/site.css')`
  - reads from the model’s asset-path map

If you need a branch such as “show this only for the French site,” shape a boolean or object in Rust
first. Do not try to invent that logic in the template.

## The Common Top-Level Request Model

Davenda’s runtime request model usually starts with keys like:

- `customer_app`
- `route_name`
- `path`
- `locale`
- `method`
- `site`
- `route_params`
- `links`
- `navigation`
- `page`
- `flashMessages`

That is why templates can usually stay simple: the runtime has already done the shaping work.

## Lists, Nested Objects, And Booleans

### Nested objects

Use objects when a group of values belongs together:

```html
<span dv:text="${site.brandName}">Brand</span>
<p dv:text="${page.summary}">Summary</p>
```

### Booleans

Use booleans for visibility and state:

```html
<section dv:if="${hasFlashMessages}">...</section>
<p dv:unless="${cartItems}">Your cart is empty.</p>
```

### Lists

Lists are always lists of child models, not raw primitives:

```html
<li dv:each="item : ${cartItems}">
  <strong dv:text="${item.title}">Fallback</strong>
</li>
```

That keeps repeated structures explicit and typed.

## Trusted HTML

`TrustedHtml` is the explicit escape hatch for pre-rendered markup.

Use it only when runtime code deliberately owns sanitization and structure. Everything else should
stay as normal text.

Practical rule:

- `RenderValue::text(...)` is normal
- `RenderValue::trusted_html(...)` is exceptional

## Asset Paths In The Model

The asset helper works because `RenderModel` carries a separate asset-path map:

```rust
model = model.with_asset_path(
    "theme/assets/site.css",
    "https://cdn.example.com/theme/assets/site.abc123.css",
)?;
```

Then templates read it like this:

```html
<link rel="stylesheet" dv:href="asset('theme/assets/site.css')" />
```

That is how templates stay readable while production still serves hashed assets.

## Where Runtime Models Come From

For request rendering, the main shaping code lives in the runtime render layer.

The runtime currently:

- injects request-level keys such as `customer_app`, `route_name`, `path`, and `locale`
- injects site and link objects
- injects published asset URLs from the active manifest
- adds route-specific data for storefront, account, admin, and other surfaces

This is the practical reason template-model docs matter: if the model is well-shaped, templates stay
small.

## Common Mistakes

### Treating the model like untyped JSON

Davenda’s model is intentionally typed. Use booleans, lists, objects, and trusted HTML for their
real purposes.

### Building asset URLs manually

Use the asset-path map through `asset('...')`.

### Pushing render-time decisions into templates

Shape booleans and nested objects in runtime code first.

### Passing raw HTML through plain text fields

Use `TrustedHtml` only when the boundary is explicitly trusted.

## Supporting Implementation And Repo Examples

Concrete supporting files:

- `crates/davenda-template/src/model/render.rs`
- `crates/davenda-runtime/src/render/model.rs`
- `apps/shoppr/templates/admin/audit.html`
- `apps/shoppr/templates/pages/home.html`
- `apps/gitly/templates/gitly/home.html`
- `apps/gitly/templates/gitly/repository.html`

## What Should I Read Next?

- [Template Language](./template-language.md)
- [Theme Asset Delivery](./theme-asset-delivery.md)
- [Internationalization](./internationalization.md)
- [Themes, Rendering, And Assets](../core-concepts/themes-rendering-and-assets.md)
