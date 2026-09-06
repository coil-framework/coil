---
title: cargo coil module add and remove
---

`cargo coil module add` and `cargo coil module remove` change the enabled official module set in
the descriptor, then re-apply the managed workspace files.

## Add Modules

```bash
cargo coil module add memberships --root ./my-store
```

or multiple at once:

```bash
cargo coil module add memberships events --root ./my-store
```

## Remove Modules

```bash
cargo coil module remove memberships --root ./my-store
```

At least one module must remain enabled.

## Typical Flow

```bash
cargo coil module add memberships --root ./my-store
cd my-store
cargo run -p my-store -- validate
```

## Read Next

- [CLI Auth And Module Commands](./cli-auth-and-modules/)
