# coil-events

`coil-events` provides the events module for Coil applications.

It is used for event catalogue, event-facing workflows, and the supporting integrations with auth, commerce, memberships, and jobs.

## Install

```toml
[dependencies]
coil-events = "0.1.0"
```

## When to use this crate directly

- You are composing Coil manually and want events support.
- You are extending or contributing to Coil’s events module.

Most applications that need events should begin with `coil-rs`.

## Related crates

- `coil-commerce`: event-linked product and transactional behaviour.
- `coil-memberships`: event access and member-aware behaviour.
- `coil-jobs`: background processing for event operations.

## Learn more

- Docs: https://coil.rs/docs/reference/modules/events
- Architecture: https://coil.rs/architecture
