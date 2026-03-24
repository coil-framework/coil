# Reference Capability Registry

**Part:** Appendices  
**Chapter:** 88

Capabilities are the stable permission contracts consumed by core services, official modules, customer apps, and WASM extensions. They are not the same thing as tuple storage, relation names, or role labels. The auth model determines how a capability is satisfied for a subject and resource. Modules only depend on the capability itself.

## Naming Rules

Capabilities use the form `domain.resource.action`.

- `domain` identifies the owning module family or cross-cutting concern
- `resource` is singular and stable across modules
- `action` is a verb or verb phrase with narrow meaning

Examples:

- `cms.page.read`
- `cms.page.publish`
- `catalog.product.edit`
- `events.booking.check_in`
- `asset.manage_storage`

Names should be explicit rather than convenient. `admin.manage` is too vague. `admin.user.manage` or `admin.audit.read` is better. Capabilities should also describe intent rather than transport. Use `order.refund.issue`, not `order.refund.post`.

## Versioning Rules

The registry is versioned because official modules depend on it.

- New capabilities may be added in a backward-compatible release.
- Removing or renaming a capability requires a major-version change.
- The meaning of an existing capability must not drift silently.
- Capability aliases may exist during a deprecation window, but the registry must identify the canonical name.

Customer apps and extensions should declare the minimum registry version they need. Official modules should publish the registry version ranges they support.

## Reference Domains

The following capabilities are part of the baseline platform vocabulary.

| Capability | Meaning | Typical owner |
| --- | --- | --- |
| `system.module.manage` | Install, enable, or disable official modules for a customer app | core/admin |
| `system.config.read` | View effective runtime configuration | core/admin |
| `system.config.write` | Change runtime configuration through approved admin tooling | core/admin |
| `admin.shell.access` | Access the shared back-office shell | admin |
| `admin.audit.read` | View audit and operational logs exposed in admin | admin |
| `cms.page.read` | View a managed page resource in non-public contexts | CMS |
| `cms.page.publish` | Transition a page into a public state | CMS |
| `cms.page.edit` | Change content or metadata for a page | CMS |
| `cms.navigation.edit` | Change menus and navigation structures | CMS |
| `catalog.product.read` | View non-public product state | commerce |
| `catalog.product.edit` | Edit product content, inventory-facing fields, or merchandising data | commerce |
| `catalog.collection.edit` | Manage categories or collections | commerce |
| `checkout.session.create` | Start a checkout flow | commerce |
| `order.read` | View order detail in admin or customer support contexts | commerce |
| `order.refund.issue` | Issue a refund or reversal | commerce |
| `membership.subscription.manage` | Create, modify, pause, or cancel subscriptions | memberships |
| `membership.tier.edit` | Change membership tier definitions | memberships |
| `events.event.publish` | Publish or unpublish an event | events |
| `events.slot.manage` | Manage time slots, capacity, or schedule state | events |
| `events.booking.create` | Create a booking or reservation | events |
| `events.booking.check_in` | Check a booking in at the event boundary | events |
| `asset.read` | Read managed assets through non-public channels | media/core |
| `asset.read_public` | Allow public delivery of a managed asset | media/core |
| `asset.publish` | Publish a managed asset into a public state | media/core |
| `asset.replace` | Replace or update an existing managed asset | media/core |
| `asset.manage_storage` | Change storage-class or delivery-policy metadata | media/core |
| `seo.metadata.edit` | Edit canonical, robots, or structured-metadata inputs | core/CMS |
| `i18n.translation.edit` | Edit platform or app translations through approved tooling | core/admin |

This list is intentionally compact. Customer apps may define additional domains, but they should avoid colliding with first-party namespaces.

## Binding Rule

Capability bindings connect capability names to authorization-model checks. The registry does not prescribe relation names. For example, a default auth package might bind `cms.page.publish` to the `publish` permission on `page`, while a customer-specific replacement package may derive the same capability from a completely different relation graph. Official modules must work with either.

## Extension Guidance

WASM extensions should request capabilities by canonical name and fail clearly if the hosting app does not provide the required binding. Extensions should not assume that an admin user, a site owner, or a group member has any specific relation name in the underlying auth model. The registry is the only supported contract at that level.
