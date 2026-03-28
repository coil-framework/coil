---
title: Configuration And Secrets
---

Davenda separates:

- customer app manifest
- platform runtime config
- secret resolution

This keeps product composition, operational configuration, and secret handling from collapsing into one opaque file.

## Typical Inputs

- database URL
- cache and jobs backend
- object storage
- auth package selection
- payment provider secrets
- observability options

## Guidance

- commit manifests and non-secret config
- resolve secrets from the environment or a secret manager
- validate config in CI and before deploy
- keep dev and production configs close enough that behavior stays believable
