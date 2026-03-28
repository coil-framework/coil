---
title: Theming, Localisation, And Accessibility
---

Gitly is the best public non-commerce example of a customer-owned frontend layer on Coil.

It is also important to read it honestly:

- theme switching is frontend-owned
- localized routes are runtime-backed
- most visible translated copy is currently applied in frontend JS, not through a server-rendered
  translation API

## Theme Switching Is A Customer-Frontend Behavior

The relevant control markup is simple:

```html
<div class="theme-switcher" role="group" aria-labelledby="theme-switcher-label">
  <button type="button" data-theme-option="light">Light</button>
  <button type="button" data-theme-option="dark">Dark</button>
  <button type="button" data-theme-option="system">System</button>
</div>
```

And the behavior is owned by the app’s JS:

```js
function applyTheme(theme) {
  const resolved = theme === "system"
    ? (window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light")
    : theme;
  document.documentElement.dataset.theme = resolved;
  localStorage.setItem("gitly-theme", theme);
}
```

That is a good public example because it keeps theming in customer assets, not in framework magic.

## Localisation Is Split Across Runtime And Frontend

Gitly still declares locales and localized routes in config:

```toml
[i18n]
default_locale = "en-GB"
supported_locales = ["en-GB", "fr-FR", "de-DE"]
localized_routes = true
```

So the runtime side is real:

- locale-prefixed routes exist
- locale-aware links exist
- the request already resolves under a locale

But the visible copy on many pages is then applied by a customer-owned frontend dictionary:

```html
<h1 data-i18n="actions.title">Workflow runs</h1>
<p data-i18n="actions.mockBody">
  This browser-side loop simulates a scheduled refresh so the Actions demo shows visible cadence.
</p>
```

```js
function applyCopy(locale) {
  const messages = translations[locale] || translations["en-GB"];
  document.querySelectorAll("[data-i18n]").forEach((node) => {
    const key = node.getAttribute("data-i18n");
    const value = messages.copy[key] || messages[key];
    if (value) node.textContent = value;
  });
}
```

That makes Gitly a good example of a customer-owned dictionary pattern, not a proof that Coil
already ships a built-in `t()` helper.

## Accessibility Is Visible In The Markup

Gitly is worth studying because it applies these behaviors in dense product pages, not only in a
marketing homepage.

For example, the Actions page already includes:

- a skip link
- labelled primary navigation
- labelled language and theme controls
- dense panels that still keep semantic headings and readable control groups

That keeps accessibility work in the same customer-owned template and asset layer as the rest of
the UI.

## What To Copy From Gitly

Copy Gitly when you want:

- customer-owned theme switching
- localized routes plus app-owned UI dictionaries
- accessible product-shell controls
- a non-commerce example of the same site/locale/runtime model

Do not copy Gitly as proof of:

- server-rendered translated copy everywhere
- a framework-owned translation file system

That is not what this app is demonstrating.

## Read Next

- [Internationalisation](../../reference/internationalization.md)
- [Theme Structure](../../reference/theme-structure.md)
- [Accessibility](../../reference/accessibility.md)
