---
title: cargo coil init
---

`cargo coil init` initialises the current directory, or a directory you pass with `--root`, as a
Coil customer workspace.

Use it when the directory already exists and should become a Coil project.

## Default Behaviour

Interactive mode is the default:

```bash
cargo coil init
```

That writes the same managed files as `cargo coil new`, but into the current directory instead of a
new one.

## Working In Another Directory

```bash
cargo coil init --root ./my-existing-folder
```

## Non-Interactive Use

```bash
cargo coil init \
  --root ./my-existing-folder \
  --no-input \
  --name my-store \
  --display-name "My Store" \
  --default-locale en-GB \
  --locale fr-FR \
  --module cms \
  --module commerce \
  --module admin
```

## `init` Versus `new`

Use `new` when:

- the target directory does not exist yet
- you want a fresh project root

Use `init` when:

- the repository already exists
- you are converting an existing folder into a Coil project

## Read Next

- [cargo coil new](./cargo-coil-new/)
- [cargo coil apply](./cargo-coil-apply/)
