---
title: Build And Deploy
---

Davenda expects a customer project to own the binary and configuration while consuming Davenda from upstream crates.

## Development

- use Docker Compose for local dependencies
- keep platform config explicit in `platform.dev.toml`
- keep app manifest and templates in the customer project
- use linked Rust for customer business logic

## Production

At minimum, you need:

- a compiled customer binary
- platform configuration
- secrets management
- Postgres
- Redis or equivalent cache/job backing
- object storage
- observability endpoints

## Recommended Flow

1. Build the customer binary.
2. Validate config.
3. Apply migrations.
4. Publish assets.
5. Start the runtime.
6. Observe health, metrics, and traces.

## Why Davenda Separates App And Platform Config

Because branding and product behavior change at a different rate from deployment and operational controls.
