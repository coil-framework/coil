---
title: cargo coil locale add
---

`cargo coil locale add` adds a locale to the descriptor and attaches it to a site.

Use it when the same site should serve another language or locale variant.

## Basic Usage

```bash
cargo coil locale add fr-FR --root ./my-store
```

By default, the command updates the default site in the descriptor.

To target a specific site:

```bash
cargo coil locale add pl-PL --root ./my-store --site eu
```

## Make It The Default For The Site

```bash
cargo coil locale add fr-FR --root ./my-store --site eu --default-for-site
```

## What It Changes

This command updates:

- `.coil/project.toml`
- `app.toml`
- `platform.dev.toml`
- `platform.toml`
- `translations/<locale>.toml`

## Read Next

- [Internationalisation](./internationalization.md)
- [cargo coil site add](./cargo-coil-site.md)
