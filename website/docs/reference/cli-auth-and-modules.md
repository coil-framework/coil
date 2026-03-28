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

Use it after `auth package validate` and `auth bindings inspect`, not before them. That sequence
tells you:

- whether the package is structurally valid
- how the stable capability maps into the current package
- whether a concrete subject/resource check passes

### `auth bindings inspect`

Use it when you want to know how a stable capability maps into the current auth package:

```bash
cargo run -p davenda-cli -- auth bindings inspect \
  --config apps/shoppr/platform.dev.toml \
  --capability cms.page.publish
```

This is the command to reach for when a module says “I need `cms.page.publish`” and you want to see
how that stable capability is represented inside the active auth package.

### `auth package validate`

Use it before rollout or when changing auth files:

```bash
cargo run -p davenda-cli -- auth package validate \
  --config apps/shoppr/platform.dev.toml
```

Run this before:

- release planning
- cutover
- changing auth schema files
- switching auth packages in a customer app

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

This is more useful than reading `app.toml` by hand because it shows the validated composed view
after dependency and runtime checks.

### `module inspect`

```bash
cargo run -p davenda-cli -- module inspect cms --config apps/shoppr/platform.dev.toml
```

Use this when you need module-level detail before changing app composition.

Typical questions this command answers:

- what routes does this module add?
- what capabilities does it require?
- what dependencies does it declare?
- what jobs or admin/operator surfaces come with it?

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

After any module change, use the customer binary as the next check:

```bash
cd apps/shoppr
cargo run -p shoppr -- validate
```

That proves the app still composes with its customer-owned templates, extensions, and linked
backend.

## Read Next

- [CLI Commands](./cli-commands.md)
- [Customer Workspace Binaries](./customer-workspace-binaries.md)
