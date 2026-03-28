---
title: Observability
---

Davenda treats observability as part of the platform contract.

You should expect:

- logs
- metrics
- tracing
- operational diagnostics

## For Product Teams

This matters because modern web products need:

- request visibility
- background job visibility
- auth and permission diagnostics
- release and migration confidence
- cutover and rollback confidence

## What To Verify In Practice

- request and error logs are structured
- metrics are exposed and scrapeable
- traces connect key product journeys
- jobs are inspectable
- operator commands surface enough state to debug safely
