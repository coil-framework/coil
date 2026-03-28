---
title: cargo coil site add
---

`cargo coil site add` adds a new site to the project descriptor and re-renders the managed app and
platform config.

Use it when one brand needs another true site boundary, not just another locale.

## Basic Usage

```bash
cargo coil site add eu \
  --root ./my-store \
  --display-name "EU Store" \
  --brand-name "My Store EU" \
  --canonical-domain eu.my-store.localhost \
  --default-locale fr-FR
```

Optional additional domains can be added with repeated `--domain` flags.

## When To Use A Site

Use a site when the new surface has a real boundary such as:

- distinct hostnames
- distinct default locale
- different merchandising or catalogue emphasis
- different operational or brand presentation needs

Do not create a site when a locale is enough. Use `cargo coil locale add` for shared-assortment
translation work.

## Read Next

- [Sites, Locales, And Markets](../core-concepts/sites-locales-and-markets.md)
- [cargo coil locale add](./cargo-coil-locale.md)
