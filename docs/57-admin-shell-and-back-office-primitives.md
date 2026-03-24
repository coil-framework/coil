# Admin Shell and Back Office Primitives

**Part:** Native Batteries  
**Chapter:** 57

The admin shell is an official native module built on top of core primitives. It is not the framework itself, but it is important enough to ship as a reusable product surface because CMS, commerce, memberships, events, and media all need the same operator foundations.

## Shared Operator Surface
The shell provides the consistent back-office frame: navigation, layout, resource routing, authentication entry points, notifications, dashboards, and the composition model for admin screens. On top of that, it supplies reusable primitives for forms, tables, filters, bulk actions, and detail views. These primitives should be accessibility-aware by default because the chat explicitly treats accessibility as a platform contract rather than as a late design pass.

## Module Contributions
Modules contribute admin resources through explicit registration, not through ambient global hooks. A module can register pages, resources, widgets, dashboards, and workflows, but it must also declare the capabilities required to see or use them. The admin shell then decides whether a given operator sees a navigation item, a filter, or an action button by consulting the shared auth layer.

Audit trails belong here as well. Back-office actions such as publishing content, refunding an order, changing a storage policy, or checking in a booking should leave a coherent trace tied to the acting user, the capability exercised, and the affected resource.

## Customer-App and Extension Participation
Customer apps may add bespoke admin resources, and WASM may contribute narrowly scoped widgets or workflow helpers where the host explicitly allows it. The important restriction is that extensions participate inside shell-defined contracts. They do not take over the admin runtime, bypass accessibility rules, or acquire broad privileged access to data and secrets.

The result is a back office that feels like one product even though it is assembled from multiple modules. That is the point of shipping an admin shell as a battery rather than letting every module grow its own private operator UI.
