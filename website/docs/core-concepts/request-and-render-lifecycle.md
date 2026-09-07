---
title: Request And Render Lifecycle
---

Coil is an HTML-first framework. That sentence is easy to repeat and easy to misunderstand.

The important point is that request handling, auth, route resolution, render-model assembly, and progressive enhancement all belong to one coherent lifecycle.

## What It Is

The request and render lifecycle is the path from an incoming HTTP request to:

- a full HTML page
- a fragment update
- a redirect after a form action
- a typed JSON response for a genuinely API-shaped route

Coil treats full pages as the default path, not as a thin fallback after API design.

## Why It Exists

Many web stacks split the product in awkward ways:

- a browser app owns most state
- server rendering is optional or bolted on
- forms and redirects feel secondary
- auth and route semantics drift between page and API layers

Coil tries to keep those concerns unified, because real products usually mix:

- public pages
- account surfaces
- admin pages
- stateful form actions
- localised routes

## The Short Version

The exact internals are deeper than this page, but the shape is consistent:

1. the runtime resolves the request against host, site, locale, and route surfaces
2. auth and capability checks run against the resolved route
3. request input is normalized and validated
4. handlers execute page, action, or API behaviour
5. a render model is assembled for page-shaped responses
6. templates render HTML using explicit data rather than arbitrary code execution

That lifecycle is what lets Coil keep HTML-first rendering without giving up operational or security discipline.

## Shoppr Home Page Example

Take a request for:

```text
GET /en-GB/pages/home
Host: www.example.com
```

In Shoppr, the runtime will effectively do this:

1. Resolve the host to the correct site.
   - In Shoppr, `www.example.com` maps to the UK site.
2. Resolve the locale.
   - `/en-GB/...` selects English (Great Britain).
3. Match the route surface.
   - The CMS page route resolves the `home` page.
4. Evaluate auth and visibility.
   - Public page reads are allowed without customer login.
5. Build the base render model.
   - Site, locale, page content, navigation, storefront context, and SEO metadata are assembled.
6. Apply customer render-model contributions.
   - Linked Rust can mount namespaced model data and merge fields into shared objects such as `page`.
7. Execute render hooks.
   - Runtime-installed extensions can contribute to the render path.
8. Render the template.
   - The template engine turns the render model into the final HTML response.

This is why the page route is not "just a template file". Host, site, locale, SEO, CMS content, auth, and extensions all participate before the final HTML is rendered.

## Shoppr Product Page Example

Now take a product detail page:

```text
GET /en-GB/shop/products/harbor-cap
Host: www.example.com
```

The flow is similar, but the render model carries commerce-specific state:

- selected site
- selected locale
- canonical and alternate URLs
- product card and detail data
- collection and related-product context
- structured product SEO metadata
- any extension or linked-backend contributions relevant to rendering

That model is prepared in Rust first, and only then rendered through the template engine. This is why the template language can stay intentionally constrained.

Here is the concrete binding path the docs should make explicit.

The commerce module contributes the page surface and template:

```rust
HttpSurfaceContribution::page(
    "commerce.product-detail",
    HttpSurfaceArea::Public,
    "/shop/products/{product_slug}",
    "commerce/product-detail",
)
.localized()
```

The runtime then assembles the common request model and appends route-specific keys for that route:

```rust
let mut model = RenderModel::new()
    .with_value("route_name", RenderValue::text(execution.route.route_name.clone()))?
    .with_value("locale", RenderValue::text(execution.locale.clone()))?
    .with_object("site", site_model(self, execution)?)?
    .with_object("links", links_model(self, execution)?)?
    .with_object("page", page_model_for_route(execution, template_name, fragment_id))?;

// later for commerce.product-detail
model = model
    .with_bool("has_product", true)?
    .with_object("product", fixture.product_for(slug))?
    .with_list("product_cards", product_cards)?;
```

And the product-detail template consumes those exact keys:

```html
<section class="product-page__hero" coil:if="${has_product}">
  <h1 coil:text="${product.name}">Harbor Cap</h1>
  <p class="product-page__price" coil:text="${product.price}">GBP 29</p>
  <a class="button" coil:attr="href=${links.cart}">Review cart</a>
</section>
```

Linked Rust can now extend that model before final render through `RenderModelHooks`, for example by:

- mounting a customer namespace such as `crm_page`
- merging extra fields into `page`

This is the actual lifecycle contract:

- the route picks the template name
- the runtime shapes the base render model
- linked Rust can contribute mount and merge operations
- the template reads only the final shaped keys

The template is not discovering data on its own. The runtime has already decided the page contract.

## Gitly Custom Route Example

Gitly demonstrates the customer-owned side of the same story.

Its app crate adds routes and maps them to template names directly:

```rust
for (route, template) in gitly_page_routes() {
    let route_name = route.name.clone();
    ensure_route(runtime, route)?;
    ensure_handler(runtime, HandlerDefinition::page(route_name, template)?)?;
}
```

With route definitions like:

```rust
("repo", "/forgeflow/platform-ui", "gitly/repository")
```

So the customer-owned part is explicit:

- Gitly defines the route
- Gitly defines the template name
- the runtime still performs the render step

Important distinction:

- Gitly still demonstrates customer-owned route-to-template mapping clearly
- customer-owned server-side page shaping is now done through linked Rust render-model hooks, not
  by an implicit or magical template-side data source

That is why route ownership and model ownership should be thought of as separate concerns.

## Stateful Action Example: Cart Update

Now look at a state-changing request:

```text
POST /cart
Host: www.example.com
```

This is still part of the same lifecycle. The runtime:

1. resolves host, site, locale, and route
2. resolves the browser session
3. validates CSRF
4. normalizes posted cart inputs
5. executes the cart mutation
6. updates runtime state
7. returns the correct HTML-first outcome
   - usually a redirect
   - sometimes a fragment response for progressive enhancement

This is what “HTML-first” means in Coil in practice. Forms and redirects are not legacy escape hatches. They are part of the primary model.

## Where Linked Rust And WASM Participate

The lifecycle is also where customization enters the runtime.

Linked customer Rust can participate by:

- shaping customer-specific business rules
- contributing to checkout or order behaviour
- handling verified webhook logic
- recording customer-specific audit evidence

Runtime-installed WASM can participate by:

- render hooks
- admin widgets
- bounded runtime extension points

That is why lifecycle understanding matters. It tells you where customization belongs.

## What "HTML-First" Means In Practice

It means:

- forms and redirects are normal
- server-rendered pages are normal
- fragments are supported for progressive enhancement
- JSON exists when the route is truly API-shaped

It does not mean:

- no interactivity
- no JavaScript
- no typed backend behaviour

## Common Mistakes

### Thinking of rendering as a late presentation step

Site, locale, auth, and module composition all affect rendering. It is not just "turn data into HTML."

### Expecting arbitrary logic in templates

Coil keeps templates deliberately constrained. Complex state should be prepared in Rust render models, not improvised inside the template engine.

### Assuming a template file alone defines the page contract

The page contract actually comes from both:

- the handler choosing the template name
- the runtime shaping the `RenderModel`

If either side is missing, the template will feel disconnected.

### Treating form actions as second-class behaviour

In Coil, stateful form flows are part of the primary model, especially for storefronts, account areas, and admin surfaces.

### Forgetting that host and locale resolution happen before rendering

If you think only in terms of route path strings, multi-site behaviour will feel confusing quickly.

## Read Next

- [Sites, locales, and markets](../sites-locales-and-markets/)
- [Template Models](../reference/template-models/)
- [Theme structure](../reference/theme-structure/)
- [Customer Rust vs third-party WASM](../reference/customer-vs-wasm/)
- [Reference overview](../reference/overview/)
