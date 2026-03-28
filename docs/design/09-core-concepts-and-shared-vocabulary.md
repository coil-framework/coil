# Core Concepts and Shared Vocabulary

The platform uses several terms in precise ways. Keeping these definitions stable matters because the architecture deliberately separates ideas that older systems tend to collapse together.

## Core

Core is the native runtime and contract layer. It owns HTTP handling, configuration, service registration, authorization infrastructure, caching, storage abstractions, rendering, observability, TLS lifecycle support, and the WASM host runtime. Core does not own every product feature.

## Official Module

An official module is a first-party native package built on top of core. Modules provide reusable product capabilities such as CMS, admin, commerce, memberships, events, media, search, or reporting. They are installable and composable rather than being hardwired into every deployment.

## Customer App

A customer app is the deployable application for one customer. It selects official modules, contributes templates and theme assets, binds capabilities, chooses deployment-wide policy such as auth and storage, and provides customer-specific configuration and extensions. A customer app may expose one or more public sites. The platform is primarily shared framework plus separate customer apps, not one giant shared tenant application.

## Site

A site is the public delivery unit inside a customer app. It owns host bindings, canonical-host policy, and locale-routing policy for incoming requests and generated URLs. A customer app may have one site or multiple sites that share the same runtime and installed module set.

## Brand

A brand is an optional identity and presentation dimension that may be attached to a site. It is not the primary routing primitive. Routing resolves site first, then any brand-aware behavior flows from that site context.

## Market

A market is a possible future commerce-oriented segmentation concept for catalog visibility, pricing, currency, tax, or fulfillment rules. It is not the first-class multi-site primitive. The platform introduces site before market so host and locale behavior have one concrete, implementable home.

## Capability

A capability is the permission contract that official modules depend on. Capabilities describe what the platform needs to decide, such as `cms.page.publish`, `catalog.product.edit`, or `asset.read_public`. Capabilities are intentionally more stable than the relation names inside any one authorization model.

## Authorization Model

The authorization model defines resource types, relations, and derived permissions for an installation. The platform ships a default model, but customer apps or developers may extend or replace it. The model is separate from both the tuple storage schema and the capability registry.

## Tuple Schema

The tuple schema is the storage structure used by the auth engine to persist and query relationships. It supports recursive CTE evaluation in Postgres, but it should not be confused with the higher-level authorization model or with module capability bindings.

## Capability Binding

A capability binding maps module capabilities onto a particular authorization model. This is what makes "use the default model," "extend the model," and "replace the model" all viable. Without capability bindings, official modules would quietly depend on hard-coded relation names and the replacement story would be fake.

## Managed Asset

A managed asset is a file or media object with business meaning: uploaded media, downloadable documents, customer-managed images, gated files, or similar content. Managed assets participate in auth, publication, and storage policy.

## Deploy Asset

A deploy asset is a build artifact such as a hashed CSS or JavaScript bundle produced by the customer app's frontend pipeline. Deploy assets are published by the build or deploy process and are generally treated as always-public deployment artifacts rather than fine-grained auth resources.

## Delivery Mode and Sync Mode

Delivery mode describes how bytes are served, such as `public_cdn`, `signed_url`, `app_proxy`, or `local_only`. Sync mode describes where the durable copy lives, such as object-store-backed or local-only. The authorization system governs whether an asset may be published or accessed publicly, while storage policy governs where the bytes live and how they are delivered.

## WASM Extension

A WASM extension is a sandboxed customization package that runs through the host API. It may contribute routes, fragments, metadata, jobs, webhook consumers, or other approved behaviors, but it does not gain raw access to internal runtime state or privileged infrastructure.

These terms are not just documentation niceties. They are the vocabulary that keeps module boundaries, auth design, and operational policy from drifting into ambiguity.
