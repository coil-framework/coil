# Customer Extensions and Integration Patterns

**Part:** Customer Apps  
**Chapter:** 80

Customer apps incorporate custom behavior primarily through WASM extensions. That is the default path for work that is specific to one customer, one deployment, or one integration and does not deserve promotion into a supported native module. The intent is to give customer apps real flexibility while keeping platform assumptions clean.

## Common Extension Patterns

The supported customization surfaces map directly to the platform's explicit extension points:

- custom pages and endpoints
- admin widgets and specialized admin data providers
- webhook consumers
- jobs and scheduled workflows
- pricing, promotion, or search adapters
- render hooks and branded content fragments

These are the right tools for customer-specific business rules and integration glue.

## Integration Discipline

Integrations should use host contracts rather than bypass them. A customer extension may need outbound HTTP, secrets, storage writes, or auth checks, but it should obtain all of those through capability-scoped host APIs.

That means:

- secrets are granted by name, not hard-coded into the package
- storage writes go through policy-aware APIs
- publication and private delivery still follow auth and storage rules
- verified webhooks enter through the host, not directly into custom code
- cache hints and metadata contributions are declarations, not direct cache-engine ownership

This keeps external-system complexity from undermining the platform's internal safety model.

## When an Integration Should Become a Module

Some integrations belong in official native modules instead of customer extensions. That is usually true when the integration:

- is expected to be reused across many customer apps
- owns important shared data models
- needs deep transaction or render-pipeline integration
- becomes operationally critical enough that sandbox limits are a liability

For example, a broadly supported Stripe payments integration is a good candidate for a native official module, while a customer-specific CRM sync or branded reporting export can remain a WASM extension.

## Keeping Customer Work Contained

Customer-specific extensions should remain namespaced to the customer app that installs them. They should not become undocumented platform dependencies for other apps, and official modules should not quietly assume they exist. If a customization starts to spread across customers, the platform should either formalize it as a shared extension contract or promote it into a supported native module.

## Example

A bookings-heavy customer app might install a custom extension set that:

- receives verified partner webhooks
- reconciles external reservation data in background jobs
- exposes a branded admin widget for exception handling
- adds a customer-specific reporting endpoint

All of that can remain in WASM as long as it works through host APIs and stays outside the core data, auth, and rendering spine. That boundary is what keeps one customer's custom work from becoming everyone else's maintenance burden.
