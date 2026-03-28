---
title: cargo coil doctor
---

`cargo coil doctor` checks whether the current workspace still matches the descriptor and whether
the generated config files remain valid Coil inputs.

Use it when you want inspection without rewriting files.

## Basic Usage

```bash
cargo coil doctor
```

or:

```bash
cargo coil doctor --root ./my-store
```

## What It Checks

`doctor` currently verifies:

- `.coil/project.toml` can be loaded and validated
- managed files still match the descriptor output
- `app.toml` can be loaded as a valid customer app manifest
- `platform.dev.toml` can be loaded as a valid platform config

## Suggested CI Use

```bash
cargo coil doctor --root ./my-store
```

If this fails, the normal follow-up is:

```bash
cargo coil apply --root ./my-store
```

## Read Next

- [cargo coil apply](./cargo-coil-apply.md)
