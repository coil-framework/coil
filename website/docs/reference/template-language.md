---
title: Template Language
---

Davenda templates are HTML with a small set of `dv:*` directives.

This page is the practical reference for the template surface that exists today in the runtime and
the checked-in customer apps.

## What Is This?

Davenda’s template language is an HTML-aware, attribute-driven rendering language used for:

- full page layouts
- reusable fragments
- module-facing page shells
- storefront, account, admin, and CMS templates
- asset references that resolve through the published asset manifest

It is implemented in `crates/davenda-template` and exercised throughout Shoppr and Gitly.

## Why Does It Exist?

Davenda needs a rendering layer that preserves a few properties at the same time:

- HTML remains readable in customer apps.
- Dynamic values are escaped by default.
- Layouts, fragments, slots, and asset resolution are first-class concepts.
- Runtime render models stay explicit instead of dissolving into arbitrary code inside templates.
- The same fragment model can be used for full pages and partial rendering paths.

The template language is intentionally not a general-purpose programming language.

## When Should I Use It?

Use Davenda templates when you are writing:

- customer-owned page shells
- storefront pages
- account and admin screens
- reusable UI fragments
- module overrides in a customer theme

Do not use templates for:

- database access
- auth or capability logic
- routing decisions
- pricing or catalog business rules
- translation lookup engines

Those belong in runtime code, module code, linked customer Rust, or frontend assets.

## How Does Davenda Resolve Templates?

Davenda resolves templates through ordered template namespaces declared in the customer app:

```toml
[theme]
active = "harbor"
template_namespaces = ["customer-app", "harbor"]
asset_roots = ["theme/assets"]
```

The runtime uses those namespaces during request rendering. In practice:

- Shoppr declares `["customer-app", "harbor"]` in `apps/shoppr/app.toml`
- Gitly declares `["customer-app", "gitly"]` in `apps/gitly/app.toml`

That lets the customer app own the final template surface without forking the framework.

## Layouts, Fragments, And File Conventions

Davenda currently distinguishes two template kinds:

- `layout`
- `fragment`

The parser treats a file as a fragment when either of these is true:

- the file contains `dv:fragment="..."`
- the file lives under `templates/components/` or `templates/fragments/`

Everything else is treated as a layout.

Practical examples:

- `apps/shoppr/templates/layouts/base.html`
- `apps/shoppr/templates/layouts/storefront.html`
- `apps/shoppr/templates/commerce/collection-grid.html`
- `apps/gitly/templates/gitly/home.html`

Why some templates carry full HTML structure:

- Davenda customer apps often own the whole public document shell
- official modules render into that shell rather than inventing private page wrappers
- `<html>`, `<head>`, and `<body>` are therefore often visible in customer templates instead of
  hidden behind a framework-only wrapper

## Supported Directives

This is the concrete directive surface parsed today in `crates/davenda-template/src/parser.rs`.

### `dv:fragment`

Marks a fragment template.

```html
<section xmlns:dv="https://davenda.dev" dv:fragment="hero">
  ...
</section>
```

Use it when the file is meant to be inserted or reused instead of being treated as a full layout.

### `dv:text`

Replaces the element’s children with escaped text from the render model.

```html
<title dv:text="${page.title}">Fallback title</title>
```

Use this for normal text output.

### `dv:utext`

Replaces the element’s children with unescaped trusted HTML.

```html
<p dv:utext="${trusted_badge}"></p>
```

Constraint:

- only use this when runtime code deliberately passes trusted HTML
- plain text is not the normal input for this path

### `dv:if`

Renders the element only when the expression is truthy.

```html
<section dv:if="${hasFlashMessages}">
  ...
</section>
```

### `dv:unless`

Renders the element only when the expression is falsey.

```html
<p dv:unless="${cartItems}">Your cart is empty.</p>
```

### `dv:each`

Repeats the element for every item in a list.

```html
<li dv:each="item : ${cartItems}">
  <strong dv:text="${item.title}">Fallback</strong>
</li>
```

Syntax:

- `item : ${collection}`

### `dv:with`

Creates local bindings for the current subtree.

```html
<section dv:with="pageTitle='Collections',showCta=true">
  ...
</section>
```

Use this to improve readability, not to build large local programs in the view layer.

### `dv:replace`

Replaces the current element with another template.

```html
<nav dv:replace="~{navigation/primary}"></nav>
```

This is one of the main composition tools in Shoppr.

### `dv:include`

Keeps the current element and replaces its children with another template.

```html
<section dv:include="~{commerce/product-grid}"></section>
```

### `dv:insert`

Supports two current patterns:

- insert another template into the current element
- fill a named slot when using a fragment-only selector

```html
<div dv:insert="~{account/summary-panels}"></div>
<main dv:insert="~{::content}"></main>
```

### `dv:slot`

Declares a named slot with optional fallback content.

```html
<main dv:slot="content">
  <p>Fallback body</p>
</main>
```

This is the main layout-to-page handoff mechanism.

### `dv:attr`

Binds one or more dynamic attributes.

```html
<a dv:attr="href=${links.home},aria-label=${navigationLabel}">Home</a>
```

Bindings are comma-separated.

### `dv:<attribute>`

Any unrecognized `dv:*` attribute becomes a dynamic binding for the real HTML attribute name.

Examples used heavily in the demos:

- `dv:href`
- `dv:src`

```html
<link rel="stylesheet" dv:href="asset('theme/assets/site.css')" />
<script defer="defer" dv:src="asset('theme/assets/site.js')"></script>
```

### `dv:block`

`dv:block` is a non-rendering wrapper. Its children render, but the tag itself does not.

Use it when you need grouping for conditions or bindings without emitting extra markup:

```html
<dv:block dv:if="${hasMembership}">
  <p>...</p>
</dv:block>
```

## Expression Forms

The expression language is intentionally small.

### Model lookups

These currently all resolve as render-model lookups:

- `${value}`
- `#{value}`
- `*{value}`

Use `${...}` unless you are preserving an existing checked-in template pattern.

Nested access uses dotted keys:

```html
<span dv:text="${site.brandName}">Brand</span>
```

### Asset lookups

These resolve through the runtime asset manifest:

- `@{theme/assets/site.css}`
- `asset('theme/assets/site.css')`
- `asset("theme/assets/site.css")`

Example:

```html
<link rel="stylesheet" dv:href="asset('theme/assets/site.css')" />
```

### Literals

Supported literals are:

- single-quoted text
- double-quoted text
- `true`
- `false`

What is not supported:

- arithmetic
- inline object creation
- inline array creation
- arbitrary function calls beyond the asset helper syntax above

## Escaping Rules

Davenda escapes by default.

Today’s concrete behavior is:

- `dv:text` escapes HTML
- dynamic attribute bindings escape HTML attribute content
- `RenderValue::text(...)` escapes when rendered
- `dv:utext` is the explicit unescaped path

That means the safe default is the normal default.

## Required, Optional, And Common Conventions

Required in practice:

- the template must be valid enough for the HTML parser
- dynamic behavior must use supported `dv:*` directives
- asset references should use the asset helper path if they need published URLs

Strongly recommended:

- include `xmlns:dv="https://davenda.dev"` on the root element for readability and consistency
- keep layouts under `templates/layouts/`
- keep reusable fragments under `templates/components/` or `templates/fragments/`

Optional:

- the `~{template :: fragment}` selector style

Current constraint on selectors:

- the parser accepts `::fragment` suffixes
- current include resolution is template-name oriented, so the suffix should be treated as selector
  sugar rather than a separate nested-fragment lookup feature

## What Files And APIs Are Involved?

Template authors should know these concrete locations:

- parser and directive behavior: `crates/davenda-template/src/parser.rs`
- runtime escaping and rendering: `crates/davenda-template/src/runtime.rs`
- request render-model assembly: `crates/davenda-runtime/src/render/model.rs`
- Shoppr layouts and fragments: `apps/shoppr/templates/`
- Gitly pages and controls: `apps/gitly/templates/`

## Working Example

This is the minimal shape you should recognize immediately:

```html
<!DOCTYPE html>
<html xmlns:dv="https://davenda.dev" dv:fragment="shell" dv:attr="lang=${locale}">
  <head>
    <title dv:text="${page.title}">Shoppr</title>
    <link rel="stylesheet" href="/theme/assets/site.css" dv:href="asset('theme/assets/site.css')" />
  </head>
  <body>
    <nav dv:replace="~{navigation/primary}"></nav>
    <main dv:slot="content"></main>
  </body>
</html>
```

Real checked-in examples:

- `apps/shoppr/templates/layouts/base.html`
- `apps/shoppr/templates/layouts/storefront.html`
- `apps/shoppr/templates/pages/home.html`
- `apps/gitly/templates/gitly/home.html`

## Common Mistakes

### Putting business logic in templates

If the template needs to decide pricing, auth, or routing policy, the runtime model is incomplete.

### Overusing `dv:utext`

If `dv:utext` starts appearing everywhere, escaping discipline has already been lost.

### Hardcoding asset URLs

Use the asset helper. Otherwise local and production behavior drift immediately.

### Treating templates like a client-side component framework

The point is explicit HTML plus small bindings, not hiding everything behind view-layer cleverness.

## What Should I Read Next?

- [Theme Structure](./theme-structure.md)
- [Internationalization](./internationalization.md)
- [Accessibility](./accessibility.md)
- [SEO](./seo.md)
- `apps/shoppr/templates/`
- `apps/gitly/templates/`
