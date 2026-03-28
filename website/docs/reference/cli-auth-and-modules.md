---
title: CLI Auth And Module Commands
---

This page covers the auth and module slices of the platform CLI.

Use these commands when you need to inspect the platform boundary itself, not just one customer app.

## Auth Commands

The platform exposes:

```text
platform auth check
platform auth bindings inspect
platform auth test-model
platform auth list
platform auth lookup
platform auth explain
platform auth package validate
platform auth package inspect
```

### `auth check`

Use it to answer one practical question:

```bash
cargo run -p davenda-cli -- auth check \
  --config apps/shoppr/platform.dev.toml \
  --subject user:alice \
  --capability cms.page.publish \
  --resource page:homepage
```

This checks the active auth package and runtime config, not just static schema files.

### `auth bindings inspect`

Use it when you want to know how a stable capability maps into the current auth package:

```bash
cargo run -p davenda-cli -- auth bindings inspect \
  --config apps/shoppr/platform.dev.toml \
  --capability cms.page.publish
```

### `auth package validate`

Use it before rollout or when changing auth files:

```bash
cargo run -p davenda-cli -- auth package validate \
  --config apps/shoppr/platform.dev.toml
```

## Module Commands

The platform exposes:

```text
platform module list
platform module inspect <module>
platform module install <module>
platform module enable <module>
platform module disable <module>
```

### `module list`

```bash
cargo run -p davenda-cli -- module list --config apps/shoppr/platform.dev.toml
```

Use this to see which modules the composed runtime knows about.

### `module inspect`

```bash
cargo run -p davenda-cli -- module inspect cms --config apps/shoppr/platform.dev.toml
```

Use this when you need module-level detail before changing app composition.

### `module enable` And `module disable`

These commands intentionally support dry-run and confirmation:

```bash
cargo run -p davenda-cli -- module enable media \
  --config apps/shoppr/platform.dev.toml \
  --dry-run
```

and:

```bash
cargo run -p davenda-cli -- module disable media \
  --config apps/shoppr/platform.dev.toml \
  --dry-run
```

That is the safe operator posture for composition-changing commands.

## Read Next

- [CLI Commands](./cli-commands.md)
- [Customer Workspace Binaries](./customer-workspace-binaries.md)
