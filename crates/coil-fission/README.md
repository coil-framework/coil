# coil-fission

`coil-fission` is the application boundary of the Fission-native Coil rewrite.

Fission owns rendering, widgets, routing, reducers, effects, SSR, islands, and
full Web applications. Coil supplies product domains and production services:
multi-site request scope, auth capability policy, PostgreSQL repositories,
transactions, durable work, media, payments, and bounded extensions.

The crate deliberately does not introduce a second UI runtime or router.
