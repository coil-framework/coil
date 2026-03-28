---
title: Composition And davenda-all
---

`davenda-all` is the convenience battery.

Use it when:

- you want the default official stack
- you are getting started quickly
- you prefer one top-level dependency while learning the platform

Use narrower crate composition when:

- you want to keep the binary surface tight
- you are building a more specialized product
- you need to reason carefully about what the customer binary actually links

The app manifest still decides what the product enables at runtime.
