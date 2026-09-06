---
title: cargo coil apply
---

`cargo coil apply` regenerates the managed workspace files from `.coil/project.toml`.

This is the reconciliation command for the project generator.

## Basic Usage

```bash
cargo coil apply
```

or:

```bash
cargo coil apply --root ./my-store
```

## When To Use It

Use `apply` after:

- editing `.coil/project.toml` directly
- running `module add` or `module remove`
- running `site add`
- running `locale add`
- pulling down descriptor changes from another branch

## Relationship To `doctor`

- `apply` fixes managed drift
- `doctor` reports managed drift

Use `doctor` when you want inspection first, then `apply` when you want to reconcile.

## Read Next

- [cargo coil doctor](./cargo-coil-doctor/)
