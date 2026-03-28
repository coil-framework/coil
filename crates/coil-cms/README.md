# coil-cms

`coil-cms` provides CMS capabilities for Coil applications.

It models content workflows, page handling, and the supporting auth and job integrations needed for editorial operations.

## Install

```toml
[dependencies]
coil-cms = "0.1.0"
```

## When to use this crate directly

- You are composing Coil manually and want CMS support without the top-level crate.
- You are extending or contributing to Coil’s content-management layer.

Most applications that want CMS support should start with `coil-rs`.

## Related crates

- `coil-auth`: policy and capability modelling for editorial workflows.
- `coil-data`: persistence used by CMS records and state.
- `coil-jobs`: background work triggered by content operations.

## Learn more

- Docs: https://coil.rs/docs/reference/modules/cms
- Architecture: https://coil.rs/architecture
