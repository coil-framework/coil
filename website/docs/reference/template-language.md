---
title: Template Language
---

Davenda templates are HTML plus a small, explicit `dv:*` directive vocabulary.

## Start With A Real Template

```html
<!DOCTYPE html>
<html xmlns:dv="https://davenda.dev" dv:attr="lang=${locale}">
  <head>
    <title dv:text="${page.title}">Fallback title</title>
    <link rel="stylesheet" href="/theme/assets/site.css" dv:href="asset('theme/assets/site.css')" />
  </head>
  <body>
    <nav dv:replace="~{navigation/primary}"></nav>
    <main dv:slot="content"></main>
  </body>
</html>
```

Annotated:

- ordinary HTML stays visible
- `dv:attr` binds `lang`
- `dv:text` replaces text with an escaped render-model value
- `dv:href` resolves a published asset URL
- `dv:replace` pulls in another template
- `dv:slot` marks where child content should land

That is the core Davenda template model in one example.

## Why Does Davenda Use This Language?

Davenda wants templates to stay:

- readable as HTML
- safe by default
- deterministic
- easy to review
- suitable for full pages and fragment rendering

It is intentionally not a general-purpose scripting language. If you need business logic, compute it
in runtime code first and pass the result into the template model.

## When Should I Use It?

Use templates for:

- customer-owned document shells
- storefront and account pages
- admin pages
- reusable fragments
- module overrides

Do not use templates for:

- auth decisions
- database access
- pricing logic
- route selection
- arbitrary function execution

## Layouts, Fragments, And File Conventions

Davenda currently distinguishes:

- `layout`
- `fragment`

A file is treated as a fragment when:

- it contains `dv:fragment="..."`
- or it lives under fragment-oriented directories such as `templates/components/` or
  `templates/fragments/`

Everything else is treated as a layout.

Why some templates include full HTML structure:

- the customer app owns the actual shell
- official modules render inside that shell
- so it is normal for customer layouts to include `<html>`, `<head>`, and `<body>`

## Directive Reference

### `dv:fragment`

Use it to mark a fragment template:

```html
<section xmlns:dv="https://davenda.dev" dv:fragment="hero">
  ...
</section>
```

### `dv:text`

Replace children with escaped text:

```html
<h1 dv:text="${page.title}">Fallback title</h1>
```

Use this for the normal text path.

### `dv:utext`

Replace children with trusted, unescaped HTML:

```html
<p dv:utext="${trusted_badge}"></p>
```

Use this rarely. It is the exception, not the default.

### `dv:if`

Render only when the value is truthy:

```html
<section dv:if="${hasFlashMessages}">
  ...
</section>
```

### `dv:unless`

Render only when the value is falsey:

```html
<p dv:unless="${cartItems}">Your cart is empty.</p>
```

### `dv:each`

Repeat for each item in a list:

```html
<li dv:each="item : ${cartItems}">
  <strong dv:text="${item.title}">Fallback</strong>
</li>
```

Syntax:

- `item : ${collection}`

### `dv:with`

Create local bindings for a subtree:

```html
<section dv:with="pageTitle='Collections',showCta=true">
  ...
</section>
```

Use it to improve readability, not to smuggle application logic into the view.

### `dv:replace`

Replace the current element with another template:

```html
<nav dv:replace="~{navigation/primary}"></nav>
```

### `dv:include`

Keep the host element and replace its children with another template:

```html
<section dv:include="~{commerce/product-grid}"></section>
```

### `dv:insert`

Use when you want the host element to stay but inserted content to fill it:

```html
<div dv:insert="~{account/summary-panels}"></div>
```

### `dv:slot`

Declare a named insertion point:

```html
<main dv:slot="content">
  <p>Fallback body</p>
</main>
```

### `dv:attr`

Bind one or more attributes dynamically:

```html
<a dv:attr="href=${links.home},aria-label=${navigationLabel}">Home</a>
```

### `dv:<attribute>`

Any unrecognized `dv:*` attribute becomes a dynamic binding for the real HTML attribute name.

The most common examples are:

- `dv:href`
- `dv:src`

```html
<link rel="stylesheet" dv:href="asset('theme/assets/site.css')" />
<script defer="defer" dv:src="asset('theme/assets/site.js')"></script>
```

### `dv:block`

`dv:block` is a non-rendering wrapper. Its children render, but the wrapper tag itself does not:

```html
<dv:block dv:if="${hasMembership}">
  <p>...</p>
</dv:block>
```

## Expressions

Davenda expressions are intentionally small.

### Model lookups

These all resolve as render-model lookups today:

- `${value}`
- `#{value}`
- `*{value}`

Nested access uses dotted keys:

```html
<span dv:text="${site.brandName}">Brand</span>
```

### Asset lookups

Supported asset syntax:

- `@{theme/assets/site.css}`
- `asset('theme/assets/site.css')`
- `asset("theme/assets/site.css")`

### Literals

Supported literals:

- single-quoted text
- double-quoted text
- `true`
- `false`

Not supported:

- arithmetic
- inline arrays or objects
- arbitrary function calls

## Escaping Rules

Davenda escapes by default.

Current behavior:

- `dv:text` escapes HTML
- dynamic attribute bindings escape attribute content
- plain `RenderValue::text(...)` is escaped when rendered
- `dv:utext` is the explicit unescaped path

## Constraints And Common Mistakes

### Putting business logic in templates

If a template needs to reason about auth or pricing, the render model is missing the right values.

### Overusing `dv:utext`

If unescaped HTML becomes the normal output path, you have already lost the safety benefit.

### Hardcoding asset URLs

Use `asset('...')` and let the runtime resolve the published URL.

### Treating templates like a client-side component framework

The point is HTML first, logic second.

## Supporting Implementation And Repo Examples

Concrete supporting files:

- `crates/davenda-template/src/parser.rs`
- `crates/davenda-template/src/runtime.rs`
- `crates/davenda-template/src/tests.rs`
- `apps/shoppr/templates/layouts/base.html`
- `apps/shoppr/templates/layouts/storefront.html`
- `apps/shoppr/templates/pages/home.html`
- `apps/gitly/templates/gitly/home.html`

## What Should I Read Next?

- [Template Models](./template-models.md)
- [Theme Structure](./theme-structure.md)
- [Internationalization](./internationalization.md)
- [Themes, Rendering, And Assets](../core-concepts/themes-rendering-and-assets.md)
