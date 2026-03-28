---
title: API And Background Work
---

Gitly demonstrates two important non-commerce patterns in Davenda:

- API-style routes
- scheduled and background work

## API-Style Routes

Gitly exposes repository-style JSON endpoints and other application-shaped APIs through the customer app.

This matters because it shows Davenda does not force every product surface into page rendering only. It simply expects API-style routes to be explicit exceptions rather than the default for everything.

## Background Work

Gitly’s Actions-style demo shows how scheduled work can be surfaced as part of the product itself while still using the same jobs and scheduler model documented for the platform generally.

## What To Read Next

- [Jobs And Schedulers](../../operations/jobs-and-schedulers.md)
- [Product Structure](./product-structure.md)
