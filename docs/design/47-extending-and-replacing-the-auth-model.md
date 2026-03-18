# Extending and Replacing the Auth Model

**Part:** Authorization and Security  
**Chapter:** 47

Customer apps are allowed to change authorization semantics, but they must do it through explicit model package boundaries. The platform supports two modes: extend the shipped default, or replace it entirely. Both are first-class. Neither is allowed to silently break installed modules.

## Extending the Default
Extension is the lower-friction path. A customer app imports the default model package, adds new resource types, adds or refines relations and permissions, and contributes any extra capability bindings its own modules need. This is the right choice when the default tenant, site, content, commerce, and asset semantics are mostly correct but the organization has extra approval layers, additional staff roles, or domain-specific resources.

Because the engine, tuple schema, and capability contracts stay the same, extension preserves compatibility with official modules while still giving the customer app room to express policy accurately.

## Full Replacement
Replacement disables the default model package and installs a complete custom one. That is a valid choice, but it comes with a hard requirement: the custom model must declare which framework capabilities it satisfies. If the app installs the CMS, media, events, or admin modules, the model must bind the capabilities those modules require. Otherwise the app configuration is incomplete and should fail validation during startup or install, not at runtime after content editors discover they cannot publish a page.

## Compatibility Rules
The compatibility contract is simple:

- core owns the engine and tuple storage
- models own relation and permission semantics
- modules own capability declarations

Official modules must never inspect relation names directly. Customer apps must never rely on modules doing so. That is the guardrail that makes replacement real instead of nominal.

Model changes are versioned and migrated like any other schema-bearing package. Extending a model may require new tuples or backfills. Replacing a model may require a staged cutover where bindings are validated, tuples are migrated, and caches are invalidated before the new model becomes active. The cost is deliberate: if authorization is central, changing it must be explicit, testable, and reversible.
