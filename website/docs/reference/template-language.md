---
title: Template Language
---

Davenda’s template language is HTML-aware and attribute-driven.

It is designed to keep templates close to valid HTML while still supporting the dynamic behavior product teams need for storefronts, account areas, admin screens, and fragment-driven updates.

## Why Davenda Uses This Model

The template engine is deliberately not a general-purpose scripting language.

That choice exists to preserve:

- readable HTML
- safe escaping by default
- analyzable templates
- deterministic override order
- fragment reuse between full pages and partial updates

Most dynamic behavior is expressed through `dv:*` attributes rather than large code blocks.

## The Main Directives

The checked-in demos and runtime surface currently use these directives heavily:

- `dv:text`
- `dv:utext`
- `dv:if`
- `dv:unless`
- `dv:each`
- `dv:with`
- `dv:replace`
- `dv:insert`
- `dv:slot`
- `dv:attr`
- `dv:href`
- `dv:src`
- `dv:fragment`

These are the important teaching directives because they are the ones you will see in Shoppr and Gitly today.

## Expression Shape

Expressions are intentionally limited. The demos use model-path expressions such as:

```html
<h1 dv:text="${page.title}">Fallback title</h1>
```

The important rule is that templates read an explicit render model. They do not open database connections, call arbitrary runtime services, or execute business logic directly.

## `dv:text`

`dv:text` replaces the element’s text content with an escaped value.

Use it when:

- you want normal user-visible text
- HTML escaping should remain enabled

Example:

```html
<span dv:text="${product.name}">Fallback product name</span>
```

## `dv:utext`

`dv:utext` inserts unescaped text or markup. Use it sparingly and only when the value is already trusted and sanitized for that context.

This is the exception, not the default.

## `dv:if` And `dv:unless`

These directives control whether an element renders.

Use them for:

- optional panels
- state-dependent controls
- admin-only or account-only pieces that are already represented in the render model

Example:

```html
<aside dv:if="${cart.hasItems}">…</aside>
<p dv:unless="${cart.hasItems}">Your cart is empty.</p>
```

## `dv:each`

`dv:each` repeats an element for a collection from the render model.

Example:

```html
<li dv:each="product : ${collection.products}">
  <span dv:text="${product.name}">Fallback</span>
</li>
```

Use it for:

- product grids
- navigation items
- order lines
- admin tables

## `dv:with`

`dv:with` defines local bindings for a template subtree. It is useful when one fragment or page wants clearer names for a small group of model values.

Use it to improve readability, not to turn the template into a miniature programming language.

## `dv:replace`

`dv:replace` renders another template or fragment in place of the current element.

This is part of Davenda’s composition model for layouts and reusable fragments. It is how page shells can reuse common pieces without copy-pasting large amounts of markup.

## `dv:insert`

`dv:insert` is similar in spirit to `dv:replace` but preserves the host element and inserts child content inside it.

Use it when the surrounding element belongs to the current template but the inner body should come from another fragment.

## `dv:slot`

`dv:slot` marks a placeholder that higher-level templates or inserted fragments can fill.

Slots are stable integration points. They are one of the mechanisms that lets official modules remain brandable without turning every page into unstructured override soup.

## `dv:attr`, `dv:href`, And `dv:src`

These directives bind attributes safely from the render model.

Use:

- `dv:attr` for general attribute maps or individual attributes
- `dv:href` when binding links
- `dv:src` when binding media or images

Example:

```html
<a href="/shop" dv:href="${links.catalog}">Shop</a>
<img src="/placeholder.jpg" dv:src="${product.imageUrl}" alt="" />
```

Keeping attribute binding explicit makes templates easier to lint and safer to reason about than string-building attribute values manually.

## `dv:fragment`

`dv:fragment` declares a named renderable fragment.

Fragments are important because Davenda uses the same fragment model for:

- full-page composition
- partial rendering
- progressive enhancement flows

That means a fragment used inside a page can often also be rendered directly in response to a fragment request.

## Supported Pattern: HTML First, Logic Second

The best Davenda templates look like ordinary HTML with targeted dynamic bindings.

Good template behavior:

- render explicit model data
- use fragments and slots for composition
- keep control flow small and obvious

Bad template behavior:

- encoding business logic in the view layer
- hiding most of the page structure behind too many replacements
- treating `dv:utext` as the normal output path

## What To Read Next

- [Theme Structure](./theme-structure.md)
- [Internationalization Reference](./internationalization.md)
- [Shoppr Storefront Structure](../use-cases/shoppr/storefront-structure.md)
