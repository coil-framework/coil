# Reference Auth Model Package

**Part:** Appendices  
**Chapter:** 89

The framework ships with a default authorization model package, but the package format is designed so customers can extend or replace that model without breaking official modules. The package contains three distinct things:

- the authorization model
- capability bindings
- migration and bootstrap material

Tuple storage is a separate concern owned by core. The package describes semantics, not the storage engine itself.

## Reference Package Layout

The exact parser syntax may evolve, but the package shape should look like this:

```text
auth/
  platform-default/
    package.toml
    model.auth
    capabilities.toml
    migrations/
    seeds/
    tests/
```

`package.toml` describes versioning and mode. `model.auth` defines resource types, relations, and derived permissions. `capabilities.toml` maps published capabilities to those permissions. `migrations/` carries model or bootstrap changes. `seeds/` establishes initial tuples such as root site ownership. `tests/` contains explainable authorization cases used by CI and by the CLI.

## Manifest Fields

The manifest should at least declare:

```toml
name = "coil-default-auth"
version = "1.0.0"
mode = "replace"
storage_schema_version = 1
model_version = 1
capability_binding_version = 1
imports = []
```

`mode` is either `replace` or `extend`.

- `replace` means the package defines the full model used by the customer app.
- `extend` means the package imports another model package and adds resources, relations, permissions, or bindings.

The three version numbers exist because storage layout, auth semantics, and capability mapping change independently.

## Default Resource Vocabulary

The shipped default package should cover the platform’s common resource types:

- `tenant`, `site`, `brand`, `storefront`
- `user`, `group`, `team`, `service_account`
- `page`, `navigation`, `product`, `collection`, `order`
- `subscription`, `membership_tier`
- `event`, `event_slot`, `booking`
- `asset`, `asset_folder`, `media_library`, `admin_module`

The default relations should be conventional rather than exhaustive: `owner`, `admin`, `editor`, `viewer`, `member`, `support`, and resource-specific publication or management permissions derived from them.

## Illustrative Model Fragment

The following fragment is intentionally schematic. It shows the kind of relation graph the package carries, not the final parser syntax:

```text
type site
  relations
    owner: user | group#member
    admin: owner | user | group#member
    editor: admin | user | group#member
    viewer: editor | user | group#member
  permissions
    view = viewer
    manage = admin

type page
  relations
    parent_site: site
    owner: user | group#member
    editor: owner | parent_site#editor
    publisher: parent_site#admin
  permissions
    read = editor
    publish = publisher
```

Official modules must not read those relation names directly. They consume capability bindings instead.

## Capability Binding File

Bindings connect the capability registry to model semantics:

```toml
[bindings."cms.page.read"]
resource_type = "page"
permission = "read"

[bindings."cms.page.publish"]
resource_type = "page"
permission = "publish"

[bindings."asset.publish"]
resource_type = "asset"
permission = "publish"
```

This is the point that makes replacement real. A customer-specific package may preserve the same capability names while using a different underlying relation graph.

## Import And Override Behavior

An extending package may:

- add new resource types
- add relations or permissions to imported resource types where the package format permits controlled extension
- add new capability bindings
- override bindings for customer-specific capabilities

An extending package should not silently redefine the meaning of canonical first-party capabilities. If a customer needs materially different behavior for an existing first-party capability, a full replacement package is clearer and easier to reason about.

## Tests And Explainability

Every auth package should ship decision tests that can be executed by the CLI and inspected through the explain API. The goal is to make questions such as “why can this editor publish an asset?” or “which relation chain grants refund authority?” answerable without reading raw tuples.

That explainability requirement is what keeps a powerful relationship-based model operationally usable.
