---
title: Extension Package Format
---

Davenda runtime-installed extensions are packaged as explicit customer-app artifacts.

They are not discovered by scanning random `.wasm` files in the repository. A valid extension needs
package metadata, a built artifact, and an installation path in the customer app.

## What This Page Covers

Use this page when you want to know:

- what files make up an extension package
- how Shoppr and Gitly ship extensions
- how an installed extension is identified and loaded
- what belongs in package metadata versus the WASM binary

For the extension execution model, read [WASM Host APIs](./wasm-host-apis.md). For deciding whether
you should use WASM at all, read [Customer Rust Vs Third-Party WASM](./customer-vs-wasm.md).

## Canonical Package Layout

The checked-in examples live here:

- `apps/shoppr/extensions/shoppr-waitlist-tools/`
- `apps/gitly/extensions/gitly-actions-scheduler/`
- `apps/gitly/extensions/gitly-community-pulse/`

A practical layout looks like this:

```text
extensions/
  gitly-actions-scheduler/
    package.toml
    src/
      lib.rs
  artifacts/
    gitly-actions-scheduler.wasm
```

The separation is intentional:

- package source remains human-maintained
- the built artifact is explicit
- the customer app can pin checksums for installed artifacts

## `package.toml`

`package.toml` is the package manifest for the extension.

It should declare enough identity and runtime information for the host to reason about the package
as an installable unit.

At minimum, the package manifest should answer:

- what is this extension called?
- which artifact belongs to it?
- which runtime surface does it expect?
- which handlers or hooks does it export?

See the live examples:

- `apps/shoppr/extensions/shoppr-waitlist-tools/package.toml`
- `apps/gitly/extensions/gitly-actions-scheduler/package.toml`
- `apps/gitly/extensions/gitly-community-pulse/package.toml`

## Built Artifacts

The compiled `.wasm` file is the runtime payload.

Gitly demonstrates checked-in artifacts:

- `apps/gitly/extensions/artifacts/gitly-actions-scheduler.wasm`
- `apps/gitly/extensions/artifacts/gitly-community-pulse.wasm`

That pattern matters because installation and verification happen against a known artifact, not an
implicit build output hidden in a target directory.

## Installation Into The Customer App

The customer app manifest and runtime build own extension installation.

In practice, Shoppr and Gitly treat extensions as customer-app assets:

- the app knows which extensions are installed
- the build knows where the artifacts live
- the runtime knows which handlers are available

That is a safer model than loading every `.wasm` file found on disk.

## Checksums And Integrity

Davenda expects runtime-installed extensions to be pinned to exact build artifacts.

That means a healthy extension story includes:

- a known artifact path
- a known checksum or integrity summary
- a deliberate install step when the artifact changes

This is why demo drift was treated as a bug earlier when the checked-in extension checksum no longer
matched the built artifact.

## What Belongs In WASM Versus The Package

The package metadata should own:

- extension identity
- version
- artifact mapping
- install-time compatibility information

The WASM binary should own:

- executable handler code
- typed host calls
- runtime behaviour

Do not encode deployment-specific assumptions into the artifact name alone.

## Distribution Model

Today the practical distribution model is customer-controlled installation.

That means:

- a customer chooses the package
- the artifact is added to the customer app
- the package is installed through the customer app's extension surface

Extensions are deliberately more constrained than linked Rust code because they are designed for
runtime-installed third-party behaviour.

## Common Mistakes

- Treating a raw `.wasm` file as a complete Davenda extension.
- Skipping package metadata.
- Letting the built artifact drift from the installed checksum.
- Using WASM for customer-owned core business logic that really belongs in linked Rust.

## Read Next

- [WASM Host APIs](./wasm-host-apis.md)
- [Customer Rust Vs Third-Party WASM](./customer-vs-wasm.md)
- [Shoppr WASM Extensions](../use-cases/shoppr/wasm-extensions.md)
- [Gitly Extensions And Host APIs](../use-cases/gitly/extensions-and-host-apis.md)
