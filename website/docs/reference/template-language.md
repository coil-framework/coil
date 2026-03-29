---
title: Template Language
---

Coil templates are HTML plus a small, explicit `coil:*` directive vocabulary.

## Start With A Real Template

```html
<!DOCTYPE html>
<html xmlns:coil="https://coil.rs" coil:attr="lang=${locale}">
  <head>
    <title coil:text="${page.title}">Fallback title</title>
    <link rel="stylesheet" href="/theme/assets/site.css" coil:href="asset('theme/assets/site.css')" />
  </head>
  <body>
    <nav coil:replace="~{navigation/primary}"></nav>
    <main coil:slot="content"></main>
  </body>
</html>
```

Annotated:

- ordinary HTML stays visible
- `coil:attr` binds `lang`
- `coil:text` replaces text with an escaped render-model value
- `coil:href` resolves a published asset URL
- `coil:replace` pulls in another template
- `coil:slot` marks where child content should land

That is the core Coil template model in one example.

## Why Does Coil Use This Language?

Coil wants templates to stay:

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

Coil currently distinguishes:

- `layout`
- `fragment`

A file is treated as a fragment when:

- it contains `coil:fragment="..."`
- or it lives under fragment-oriented directories such as `templates/components/` or
  `templates/fragments/`

Everything else is treated as a layout.

Why some templates include full HTML structure:

- the customer app owns the actual shell
- official modules render inside that shell
- so it is normal for customer layouts to include `<html>`, `<head>`, and `<body>`

## Directive Reference

### `coil:fragment`

Use it to mark a fragment template:

```html
<section xmlns:coil="https://coil.rs" coil:fragment="hero">
  ...
</section>
```

### `coil:text`

Replace children with escaped text:

```html
<h1 coil:text="${page.title}">Fallback title</h1>
```

Use this for the normal text path.

### `coil:utext`

Replace children with trusted, unescaped HTML:

```html
<p coil:utext="${trusted_badge}"></p>
```

Use this rarely. It is the exception, not the default.

### `coil:if`

Render only when the value is truthy:

```html
<section coil:if="${has_flash_messages}">
  ...
</section>
```

### `coil:unless`

Render only when the value is falsey:

```html
<p coil:unless="${cart_items}">Your cart is empty.</p>
```

### `coil:each`

Repeat for each item in a list:

```html
<li coil:each="item : ${cart_items}">
  <strong coil:text="${item.title}">Fallback</strong>
</li>
```

Syntax:

- `item : ${collection}`

### `coil:with`

Create local bindings for a subtree:

```html
<section coil:with="page_title='Collections',show_cta=true">
  ...
</section>
```

Use it to improve readability, not to smuggle application logic into the view.

### `coil:replace`

Replace the current element with another template:

```html
<nav coil:replace="~{navigation/primary}"></nav>
```

### `coil:include`

Keep the host element and replace its children with another template:

```html
<section coil:include="~{commerce/product-grid}"></section>
```

### `coil:insert`

Use when you want the host element to stay but inserted content to fill it:

```html
<div coil:insert="~{account/summary-panels}"></div>
```

### `coil:slot`

Declare a named insertion point:

```html
<main coil:slot="content">
  <p>Fallback body</p>
</main>
```

### `coil:attr`

Bind one or more attributes dynamically:

```html
<a coil:attr="href=${links.home},aria-label=${navigation_label}">Home</a>
```

### `coil:<attribute>`

Any unrecognized `coil:*` attribute becomes a dynamic binding for the real HTML attribute name.

The most common examples are:

- `coil:href`
- `coil:src`

```html
<link rel="stylesheet" coil:href="asset('theme/assets/site.css')" />
<script defer="defer" coil:src="asset('theme/assets/site.js')"></script>
```

### `coil:block`

`coil:block` is a non-rendering wrapper. Its children render, but the wrapper tag itself does not:

```html
</?coil:block coil:if="${has_membership}">
  <p>...</p>
</?coil:block>
```

## Expressions

Coil expressions are intentionally small.

That means the template language currently supports only four expression categories:

- model lookups
- asset lookups
- string literals
- boolean literals

It does **not** support a general expression language with arithmetic, filters, chained function
calls, or inline object construction.

### Model lookups

These all resolve as render-model lookups today:

- `${value}`
- `#{value}`
- `*{value}`

Important: these three forms are currently equivalent aliases.

Today they all parse to the same model-key lookup. They do **not** mean different scopes or access
rules.

Preferred style:

- use `${...}` for normal model lookups

That keeps templates easier to read and avoids implying distinctions that do not currently exist.

Nested access uses dotted keys:

```html
<span coil:text="${site.brand_name}">Brand</span>
```

This is the normal lookup style you should expect to use in real templates.

### Asset lookups

Supported asset syntax:

- `@{theme/assets/site.css}`
- `asset('theme/assets/site.css')`
- `asset("theme/assets/site.css")`

Important: these three forms are also currently equivalent aliases.

Today they all resolve to the same asset-path lookup. There is no runtime semantic difference
between them.

Preferred style:

- use `asset('...')` for asset lookups

That makes the intent obvious to readers and distinguishes asset resolution from normal model
resolution.

Example:

```html
<link rel="stylesheet" coil:href="asset('theme/assets/site.css')" />
<script defer="defer" coil:src="asset('theme/assets/site.js')"></script>
```

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

Coil escapes by default.

Current behaviour:

- `coil:text` escapes HTML
- dynamic attribute bindings escape attribute content
- plain `RenderValue::text(...)` is escaped when rendered
- `coil:utext` is the explicit unescaped path

## Constraints And Common Mistakes

### Putting business logic in templates

If a template needs to reason about auth or pricing, the render model is missing the right values.

### Overusing `coil:utext`

If unescaped HTML becomes the normal output path, you have already lost the safety benefit.

### Hardcoding asset URLs

Use `asset('...')` and let the runtime resolve the published URL.

### Treating templates like a client-side component framework

The point is HTML first, logic second.

## Supporting Implementation And Repo Examples

Concrete supporting files:

- `crates/coil-template/src/parser.rs`
- `crates/coil-template/src/runtime.rs`
- `crates/coil-template/src/tests.rs`
- `apps/shoppr/templates/layouts/base.html`
- `apps/shoppr/templates/layouts/storefront.html`
- `apps/shoppr/templates/pages/home.html`
- `apps/gitly/templates/gitly/home.html`

## What Should I Read Next?

- [Template Models](./template-models.md)
- [Theme Structure](./theme-structure.md)
- [Internationalisation](./internationalization.md)
- [Themes, Rendering, And Assets](../core-concepts/themes-rendering-and-assets.md)
