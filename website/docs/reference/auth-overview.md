---
title: Auth
---

Davenda auth has four layers:

- the Zanzibar-style core engine
- the auth package
- the auth schema and capability bindings
- runtime selection in platform config

Read the auth reference in this order:

1. [Zanzibar And Core Auth](./auth-zanzibar.md)
2. [Auth Packages](./auth-packages.md)
3. [Auth Schema](./auth-schema.md)
4. [Custom Auth Schema Guidance](./custom-auth-schema.md)

Use this section when you need to answer questions like:

- what does `auth.package = "shoppr-auth"` actually select?
- what lives in `package.toml`, `model.auth`, and `capabilities.toml`?
- how do capabilities relate to relations and permissions?
- when should a customer app extend the default auth model instead of replacing it?

One rule matters throughout:

Official modules depend on capabilities, not relation names.

That rule is what makes custom auth models real instead of cosmetic.
