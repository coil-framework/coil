---
title: Template Models
---

Davenda templates render against a typed `RenderModel`, not an unstructured JSON blob.

## How A Route Actually Reaches A Template

The missing connection for most readers is usually not "what is a `RenderModel`?" but "where does
this model come from, and how does it get tied to a template?"

In Davenda, those are two separate decisions:

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
            .with_bool("hasProduct", true)?
            .with_object("product", fixture.product_for(slug))?
            .with_bool("hasProductCards", !product_cards.is_empty())?
            .with_list("productCards", product_cards)?;
    }
}
```

Finally, the template consumes those exact keys:

```html
<section class="product-page__hero" dv:if="${hasProduct}">
  <h1 dv:text="${product.name}">Harbor Cap</h1>
  <p class="product-page__price" dv:text="${product.price}">GBP 29</p>
  <p dv:text="${product.summary}">Product summary</p>
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
    ("repo", "/octocorp/platform-ui", "gitly/repository"),
    ("issues", "/octocorp/platform-ui/issues", "gitly/issues"),
];
```

So for Gitly the template tie-in is clear:

- route `gitly.en.repo`
- handler `HandlerDefinition::page(...)`
- template `gitly/repository`

### Important current boundary

The checked-in demos do **not** currently show the same thing on the server side:

- Shoppr demonstrates server-shaped route-specific models very clearly
- Gitly demonstrates customer-owned route-to-template mapping very clearly
- Gitly does **not** currently demonstrate a separate customer-app-owned Rust function that builds
  a custom server-side `RenderModel` for `gitly/repository`

In practice that means the current demos show two patterns:

- Shoppr
  - official module routes with runtime-owned server-side model shaping
- Gitly
  - customer-owned routes and template selection, with page-specific demo data expressed mostly as
    static markup, `data-*` attributes, and client-side enhancement

If you are looking specifically for "where does my customer Rust build a custom page model and bind
it to my custom template?", the current docs must answer honestly: the route-to-template part is
demonstrated in Gitly, but the checked-in custom server-side model-building example is still most
visible in the runtime-owned Shoppr route bindings.

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

## What The Demos Prove Today

Use the demos for these two different lessons:

- Shoppr
  - shows how a real server-side route ends up with strongly shaped keys such as `product`,
    `collection`, `cartItems`, `cartSummary`, and `page`
- Gitly
  - shows how a customer app adds its own routes and maps them to template names such as
    `gitly/repository` and `gitly/actions`

That distinction matters because readers often expect one demo to show both at once. Today the two
examples split the lesson across the repo.

## Common Mistakes

### Treating the model like untyped JSON

Davenda’s model is intentionally typed. Use booleans, lists, objects, and trusted HTML for their
real purposes.

### Building asset URLs manually

Use the asset-path map through `asset('...')`.

### Pushing render-time decisions into templates

Shape booleans and nested objects in runtime code first.

### Assuming every page template in the demos is backed by a customer-owned server-side model builder

That is not what the current examples do.

Shoppr's strongest examples are runtime-shaped official-module routes. Gitly's strongest examples
are customer-owned route and template registration. Read them that way.

### Passing raw HTML through plain text fields

Use `TrustedHtml` only when the boundary is explicitly trusted.

## Supporting Implementation And Repo Examples

Full implementation:

- `crates/davenda-template/src/model/render.rs`
- `crates/davenda-runtime/src/render/model.rs`
- `crates/davenda-runtime/src/render/mod.rs`
- `crates/davenda-commerce/src/module/platform/manifest.rs`
- `apps/shoppr/templates/admin/audit.html`
- `apps/shoppr/templates/commerce/product-detail.html`
- `apps/shoppr/templates/pages/home.html`
- `apps/gitly/crates/gitly-app/src/lib.rs`
- `apps/gitly/templates/gitly/home.html`
- `apps/gitly/templates/gitly/repository.html`

## What Should I Read Next?

- [Template Language](./template-language.md)
- [Theme Asset Delivery](./theme-asset-delivery.md)
- [Internationalization](./internationalization.md)
- [Themes, Rendering, And Assets](../core-concepts/themes-rendering-and-assets.md)
