---
title: Template Models
---

Coil templates render against a typed `RenderModel`, not an unstructured JSON blob.

If you are trying to understand where that model comes from, read
[Render pipeline and model composition](../core-concepts/render-pipeline-and-model-composition/)
alongside this page.

## How A Route Actually Reaches A Template

The missing connection for most readers is usually not "what is a `RenderModel`?" but "where does
this model come from, and how does it get tied to a template?"

In Coil, those are two separate decisions:

1. a route chooses a template name
2. the runtime builds a `RenderModel` for that route execution

The final HTML render only happens after both of those are known.

### Shoppr: official route -> template -> typed model

Shoppr's product-detail page is the clearest checked-in example.

The commerce module contributes the route surface and the page template name:

```rust
RouteSurface::new(
    "commerce.product-detail",
    RouteSurfaceKind::FrontendPage,
    "/shop/products/{product_slug}",
)
.localized()

HttpSurfaceContribution::page(
    "commerce.product-detail",
    HttpSurfaceArea::Public,
    "/shop/products/{product_slug}",
    "commerce/product-detail",
)
.localized()
```

That means:

- the route name is `commerce.product-detail`
- the template name is `commerce/product-detail`

When a request hits that route, the runtime renders it like this:

```rust
let selector = templates::template_selector(&page.template)?;
let model = self.render_model_for_execution(execution, &page.template, None)?;
```

Then the render-model builder adds the shared request keys first:

```rust
let mut model = RenderModel::new()
    .with_value("customer_app", RenderValue::text(execution.customer_app.clone()))?
    .with_value("route_name", RenderValue::text(execution.route.route_name.clone()))?
    .with_value("locale", RenderValue::text(execution.locale.clone()))?
    .with_object("site", site_model(self, execution)?)?
    .with_object("links", links_model(self, execution)?)?
    .with_object("page", page_model_for_route(execution, template_name, fragment_id))?;
```

After that, route-specific bindings add the product-specific fields:

```rust
"commerce.product-detail" => {
    let slug = params
        .get("product_slug")
        .map(String::as_str)
        .unwrap_or("harbor-cap");
    if catalog.visible_product_for_site(site_id, slug).is_some() {
        let product_cards = fixture.related_product_cards_for_product(slug);
        model = model
            .with_bool("has_product", true)?
            .with_object("product", fixture.product_for(slug))?
            .with_bool("has_product_cards", !product_cards.is_empty())?
            .with_list("product_cards", product_cards)?;
    }
}
```

Finally, the template consumes those exact keys:

```html
<section class="product-page__hero" coil:if="${has_product}">
  <h1 coil:text="${product.name}">Harbor Cap</h1>
  <p class="product-page__price" coil:text="${product.price}">GBP 29</p>
  <p coil:text="${product.summary}">Product summary</p>
</section>
```

This is the full binding story:

- route name chooses the response contract
- template name chooses the template file
- render-model shaping code decides which keys exist
- the template only reads those keys

### Gitly: customer route -> template mapping

Gitly demonstrates a different part of the story.

Its customer app crate adds routes and page handlers directly:

```rust
for (route, template) in gitly_page_routes() {
    let route_name = route.name.clone();
    ensure_route(runtime, route)?;
    ensure_handler(runtime, HandlerDefinition::page(route_name, template)?)?;
}
```

And the page-route table makes the template mapping explicit:

```rust
let pages = [
    ("home", "", "gitly/home"),
    ("explore", "/explore", "gitly/explore"),
    ("repo", "/forgeflow/platform-ui", "gitly/repository"),
    ("issues", "/forgeflow/platform-ui/issues", "gitly/issues"),
];
```

So for Gitly the template tie-in is clear:

- route `gitly.en.repo`
- handler `HandlerDefinition::page(...)`
- template `gitly/repository`

### Important current boundary

There are now two supported server-side model-shaping lanes:

- runtime-owned route bindings
  - official modules shape the common storefront, account, admin, and operations models
- customer-owned render-model hooks
  - linked Rust can mount new top-level namespaces and merge fields into shared objects such as
    `page`

That means the answer to "where does my customer Rust build a custom page model and bind it to my
custom template?" is now explicit:

1. your app still chooses the route and template
2. Coil builds the standard request model
3. your linked Rust render-model hook contributes additional model data
4. the template consumes the combined model

Read the full contract in [Render Model Hooks](./render-model-hooks/).

## Start With A Model And Its Template

Imagine runtime code shaping this model:

```rust
let model = RenderModel::new()
    .with_value("locale", RenderValue::text("en-GB"))?
    .with_object(
        "site",
        RenderModel::new()
            .with_value("brand_name", RenderValue::text("Shoppr"))?
            .with_value("display_name", RenderValue::text("Shoppr UK"))?,
    )?
    .with_bool("has_flash_messages", true)?
    .with_list(
        "flash_messages",
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
<html xmlns:coil="https://coil.rs" coil:attr="lang=${locale}">
  <head>
    <link rel="stylesheet" coil:href="asset('theme/assets/site.css')" />
  </head>
  <body>
    <h1 coil:text="${site.brand_name}">Brand</h1>
    <section coil:if="${has_flash_messages}">
      <article coil:each="message : ${flash_messages}">
        <p coil:text="${message.text}">Fallback</p>
      </article>
    </section>
  </body>
</html>
```

That is the core contract:

- runtime shapes typed values
- templates read those values declaratively

What templates do **not** do automatically:

- load CMS content instances by themselves
- turn content schema into `page.blocks`
- infer customer-owned request data because a field name appears in markup
- fetch live data for dynamic sections on their own

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

That typed model is the request-time output of the render pipeline, not a direct dump of app
manifest files or content schema definitions.

## How Templates Consume The Model

These are the important rules:

- `${page.title}`
  - reads nested object keys
- `coil:if="${has_flash_messages}"`
  - expects a boolean
- `coil:each="entry : ${audit_entries}"`
  - expects a list of child models
- `asset('theme/assets/site.css')`
  - reads from the model’s asset-path map

If you need a branch such as “show this only for the French site,” shape a boolean or object in Rust
first when that keeps the page contract simpler. Coil now also supports narrow view-level comparisons
and block dispatch, so simple checks like `${site.locale == 'fr-FR'}` and `${block.type == 'hero_section'}`
are valid when the branch genuinely belongs in the template.

## The Common Top-Level Request Model

Coil’s runtime request model usually starts with keys like:

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
- `flash_messages`

That is why templates can usually stay simple: the runtime has already done the shaping work.

The important boundary is that Coil only renders what the request-time model exposes. If `page`,
`product`, or `page.blocks` are missing, the template does not go and compute them.

## Lists, Nested Objects, And Booleans

### Nested objects

Use objects when a group of values belongs together:

```html
<span coil:text="${site.brand_name}">Brand</span>
<p coil:text="${page.summary}">Summary</p>
```

## Runtime Block Dispatch In `coil:each`

When a list item exposes a `type` field, Coil now augments that item at render time so the template
can branch or dispatch on the block variant without extra customer-side shaping code.

Given this model:

```rust
let page = RenderModel::new().with_list(
    "blocks",
    vec![
        RenderModel::new()
            .with_value("type", RenderValue::text("hero_section"))?
            .with_object(
                "fields",
                RenderModel::new()
                    .with_value("title", RenderValue::text("Hero title"))?,
            )?,
    ],
)?;
```

this template:

```html
<div coil:each="block : ${blocks}">
  <section coil:if="${block.is_hero_section}" coil:text="${block.fields.title}">
    Fallback
  </section>
</div>
```

can read the derived block variant directly.

Inside the loop item, Coil adds:

- `block.is_<type>`
- `block.render_fragment`
- `block.render_fragment_shared`

For a `pages/home` template and a block type of `hero_section`, those resolve to:

- `block.is_hero_section = true`
- `block.render_fragment = "pages/home/blocks/hero_section"`
- `block.render_fragment_shared = "blocks/hero_section"`

That means the model reaching the template is richer than the raw list entry. Coil is shaping a
render-oriented view of the block at render time.

That does not mean Coil automatically resolved the block from schema. It means the request-time
model already included block-shaped data and the template runtime added render-oriented helpers on
top.

## Rendering Block Fragments By Type

Once a block loop item exposes `render_fragment`, the template can dispatch directly into a fragment:

```html
<div coil:each="block : ${page.blocks}">
  <coil:block coil:replace-fragment="${block.render_fragment}"></coil:block>
</div>
```

That is the preferred pattern for CMS-style page builders because it keeps each block variant in its
own fragment file instead of building one very large `home.html`.

Recommended fragment layout for a `pages/home` template:

- `templates/pages/home.html`
- `templates/pages/home/blocks/hero_section.html`
- `templates/pages/home/blocks/featured_events.html`
- `templates/blocks/<type>.html` for shared fallbacks when multiple pages reuse the same block

Use `coil:switch` when you want the branching inline. Use `render_fragment` when the block wants its
own template.

### Booleans

Use booleans for visibility and state:

```html
<section coil:if="${has_flash_messages}">...</section>
<p coil:unless="${cart_items}">Your cart is empty.</p>
```

### Lists

Lists are always lists of child models, not raw primitives:

```html
<li coil:each="item : ${cart_items}">
  <strong coil:text="${item.title}">Fallback</strong>
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
<link rel="stylesheet" coil:href="asset('theme/assets/site.css')" />
```

That is how templates stay readable while production still serves hashed assets.

## Where Runtime Models Come From

For request rendering, the main shaping code lives in the runtime render layer.

The runtime currently:

- injects request-level keys such as `customer_app`, `route_name`, `path`, and `locale`
- injects site and link objects
- injects published asset URLs from the active manifest
- adds route-specific data for storefront, account, admin, and other surfaces
- applies linked Rust render-model contributions before template render

That final step is the important customer handoff:

- `mount("crm_page", ...)`
  - creates a new namespaced top-level object
- `merge("page", ..., MergePolicy::FailOnConflict)`
  - extends the runtime-owned `page` object without silently overwriting keys

Example:

```rust
let overlay = RenderModel::new()
    .with_value("render_source", RenderValue::text("linked-rust"))?;

let contributions = vec![
    RenderModelContribution::merge("page", overlay, MergePolicy::FailOnConflict)?,
];
```

This is the practical reason template-model docs matter: if the model is well-shaped, templates stay
small.

## Customer Namespaces Vs Framework Namespaces

Coil already owns shared namespaces such as:

- `page`
- `site`
- `links`
- `navigation`

Customer code should generally mount its own namespaces for customer-owned request data:

- `crm_page`
- `campaigns`
- `customer_extension`

Merge into framework-owned objects only when the field genuinely belongs to a shared public
contract. For that rule, read [Render model hooks](./render-model-hooks/).

## Dynamic Blocks And Live Sections

If a template renders a block list or a live section, remember that the template is still only
rendering the final request-time values it was given.

Coil does not automatically turn:

- a block schema
- a page-builder declaration
- or a content model entry in `app.toml`

into a fully resolved live block payload.

For that boundary, read
[Dynamic blocks and live-data sections](../core-concepts/dynamic-blocks-and-live-data-sections/)
and [CMS page builder model](./cms-page-builder-model/).

## What The Demos Prove Today

Use the demos for these two different lessons:

- Shoppr
  - shows how a real server-side route ends up with strongly shaped keys such as `product`,
    `collection`, `cart_items`, `cart_summary`, and `page`
- Gitly
  - shows how a customer app adds its own routes and maps them to template names such as
    `gitly/repository` and `gitly/actions`
- linked Rust render-model hooks
  - show the supported customer-owned server-side handoff for mounting and merging page data before
    render

That distinction matters because different examples prove different parts of the contract. The
official modules prove runtime-owned shaping, while render-model hooks are the customer-owned
extension point.

## Common Mistakes

### Treating the model like untyped JSON

Coil’s model is intentionally typed. Use booleans, lists, objects, and trusted HTML for their
real purposes.

### Building asset URLs manually

Use the asset-path map through `asset('...')`.

### Pushing render-time decisions into templates

Shape booleans and nested objects in runtime code first.

### Assuming customer page shaping is implicit

It is not implicit. Customer-owned server-side page shaping now goes through
`RenderModelHooks::contribute_render_model(...)`.

Shoppr's strongest examples are runtime-shaped official-module routes. Gitly's strongest examples
are customer-owned route and template registration. Read them that way.

### Passing raw HTML through plain text fields

Use `TrustedHtml` only when the boundary is explicitly trusted.

## Supporting Implementation And Repo Examples

Full implementation:

- `crates/coil-template/src/model/render.rs`
- `crates/coil-runtime/src/render/model.rs`
- `crates/coil-runtime/src/render/mod.rs`
- `crates/coil-commerce/src/module/platform/manifest.rs`
- `apps/shoppr/templates/admin/audit.html`
- `apps/shoppr/templates/commerce/product-detail.html`
- `apps/shoppr/templates/pages/home.html`
- `apps/gitly/crates/gitly-app/src/lib.rs`
- `apps/gitly/templates/gitly/home.html`
- `apps/gitly/templates/gitly/repository.html`

## What Should I Read Next?

- [Template Language](./template-language/)
- [Theme Asset Delivery](./theme-asset-delivery/)
- [Internationalisation](./internationalization/)
- [Themes, Rendering, And Assets](../core-concepts/themes-rendering-and-assets/)
- [Content schema vs content instances](../core-concepts/content-schema-vs-content-instances/)
- [Render pipeline and model composition](../core-concepts/render-pipeline-and-model-composition/)
- [Getting Started: Add Dynamic Blocks](../getting-started/add-dynamic-blocks/)
